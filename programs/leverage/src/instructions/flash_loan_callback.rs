use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    hash::hash,
};
use anchor_spl::token::{Token, TokenAccount, Mint, TransferChecked};
use omnipair::{FlashLoanCallbackData, SwapArgs, AdjustCollateralArgs, AdjustDebtArgs};
use crate::{
    constants::*,
    errors::LeverageError,
    instruction_math::callback_swap_token0_is_input,
    types::InternalCallbackData,
};

/// Accounts for the flashloan callback.
/// Field order must match exactly what omnipair's flashloan passes to the receiver program:
///   initiator, receiver_token0_account, receiver_token1_account,
///   token0_mint, token1_mint, repay0_vault, repay1_vault,
///   [remaining_accounts forwarded from multiply/close_multiply],
///   token_program  ← last, appended by omnipair
#[derive(Accounts)]
pub struct FlashLoanCallback<'info> {
    pub initiator: Signer<'info>,

    #[account(mut)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    pub token0_mint: Account<'info, Mint>,
    pub token1_mint: Account<'info, Mint>,

    #[account(mut)]
    pub repay0_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub repay1_vault: Account<'info, TokenAccount>,

    // remaining_accounts[0..11] accessed via ctx.remaining_accounts
    // token_program is the LAST account (appended by omnipair after remaining_accounts)
    pub token_program: Program<'info, Token>,
}

/// Called by omnipair mid-flashloan. Never called directly by users.
///
/// Open path:  swap → add_collateral → borrow → repay flashloan
/// Close path: repay debt → remove_collateral → swap back → repay flashloan
pub fn handle<'info>(
    mut ctx: Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
    callback_data: FlashLoanCallbackData,
) -> Result<()> {
    let InternalCallbackData {
        is_close,
        is_lev_collateral0,
        swap_amount_in,
        min_amount_out,
        repay_amount,
    } = InternalCallbackData::try_from_slice(&callback_data.data)
        .map_err(|_| LeverageError::InvalidCallbackData)?;

    let ra = ctx.remaining_accounts;
    let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
    let event_authority  = &ra[IDX_EVENT_AUTHORITY];

    let user          = ctx.accounts.initiator.to_account_info();
    let token_program = ctx.accounts.token_program.to_account_info();

    // Direction selector:
    //   open:  lev_collateral → position token
    //   close: position token → lev_collateral (reversed)
    //
    // XOR of is_lev_collateral0 and is_close gives the "is token0 the swap-in" flag:
    //   open,  is_lev_collateral0=true  → token0 in, token1 out
    //   open,  is_lev_collateral0=false → token1 in, token0 out
    //   close, is_lev_collateral0=true  → token1 in, token0 out (reversed)
    //   close, is_lev_collateral0=false → token0 in, token1 out (reversed)
    let token0_in = callback_swap_token0_is_input(is_lev_collateral0, is_close);

    let (token_in_mint, token_out_mint, user_token_in_ai, user_token_out_ai) =
        if token0_in {
            (
                ctx.accounts.token0_mint.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                ctx.accounts.receiver_token0_account.to_account_info(),
                ctx.accounts.receiver_token1_account.to_account_info(),
            )
        } else {
            (
                ctx.accounts.token1_mint.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                ctx.accounts.receiver_token1_account.to_account_info(),
                ctx.accounts.receiver_token0_account.to_account_info(),
            )
        };

    if is_close {
        handle_close(
            &mut ctx, ra, omnipair_program, event_authority,
            &user, &token_program,
            token0_in, is_lev_collateral0,
            token_in_mint, token_out_mint,
            user_token_in_ai, user_token_out_ai,
            min_amount_out, repay_amount,
        )
    } else {
        handle_open(
            &mut ctx, ra, omnipair_program, event_authority,
            &user, &token_program,
            token0_in, is_lev_collateral0,
            token_in_mint, token_out_mint,
            user_token_in_ai, user_token_out_ai,
            swap_amount_in, min_amount_out, repay_amount,
        )
    }
}

// ── Close path ────────────────────────────────────────────────────────────────
//
// After flashloan: user_token_out_ai holds debt_amount of lev_collateral.
//
// 1. repay full debt → debt = 0
// 2. remove all collateral → user_token_in_ai receives position token
// 3. swap position token → lev_collateral → user_token_out_ai receives payout
// 4. send repay_amount of lev_collateral to repay vault
// Net: user_token_out_ai retains margin + PnL - flashloan fee
#[allow(clippy::too_many_arguments)]
fn handle_close<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
    ra: &[AccountInfo<'info>],
    omnipair_program: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    user: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token0_in: bool,
    is_lev_collateral0: bool,
    token_in_mint: AccountInfo<'info>,
    token_out_mint: AccountInfo<'info>,
    user_token_in_ai: AccountInfo<'info>,
    user_token_out_ai: AccountInfo<'info>,
    min_amount_out: u64,
    repay_amount: u64,
) -> Result<()> {
    // 1. Repay full debt (u64::MAX = repay all)
    cpi_adjust_debt(
        omnipair_program, event_authority, ra, user, token_program,
        ra[IDX_TOKEN_OUT_VAULT].to_account_info(), // lev_collateral reserve vault
        user_token_out_ai.clone(),                 // lev_collateral user account
        token_out_mint.clone(),
        b"global:repay",
        u64::MAX,
    )?;

    // 2. Remove all collateral (u64::MAX = withdraw all)
    let balance_before_remove = if token0_in {
        ctx.accounts.receiver_token0_account.amount
    } else {
        ctx.accounts.receiver_token1_account.amount
    };

    cpi_adjust_collateral(
        omnipair_program, event_authority, ra, user, token_program,
        ra[IDX_COLLATERAL_VAULT].to_account_info(), // position token collateral vault
        user_token_in_ai.clone(),                   // position token user account
        token_in_mint.clone(),
        b"global:remove_collateral",
        u64::MAX,
    )?;

    // 3. Compute how much position token was withdrawn
    let amount_in_swap = if token0_in {
        ctx.accounts.receiver_token0_account.reload()?;
        ctx.accounts.receiver_token0_account.amount
            .checked_sub(balance_before_remove)
            .ok_or(LeverageError::SwapFailed)?
    } else {
        ctx.accounts.receiver_token1_account.reload()?;
        ctx.accounts.receiver_token1_account.amount
            .checked_sub(balance_before_remove)
            .ok_or(LeverageError::SwapFailed)?
    };
    require!(amount_in_swap > 0, LeverageError::SwapFailed);

    // 4. Swap position token → lev_collateral
    cpi_swap(
        omnipair_program, event_authority, ra, user, token_program,
        user_token_in_ai.clone(),
        user_token_out_ai.clone(),
        token_in_mint.clone(),
        token_out_mint.clone(),
        amount_in_swap,
        min_amount_out,
    )?;

    // 5. Send repay_amount of lev_collateral to flashloan repay vault
    let (repay_vault, repay_mint, repay_decimals) = if is_lev_collateral0 {
        (ctx.accounts.repay0_vault.to_account_info(), ctx.accounts.token0_mint.to_account_info(), ctx.accounts.token0_mint.decimals)
    } else {
        (ctx.accounts.repay1_vault.to_account_info(), ctx.accounts.token1_mint.to_account_info(), ctx.accounts.token1_mint.decimals)
    };
    anchor_spl::token::transfer_checked(
        CpiContext::new(
            token_program.clone(),
            TransferChecked {
                from:      user_token_out_ai,
                mint:      repay_mint,
                to:        repay_vault,
                authority: user.clone(),
            },
        ),
        repay_amount,
        repay_decimals,
    )
}

// ── Open path ─────────────────────────────────────────────────────────────────
//
// After flashloan: user_token_in_ai holds borrow_amount of lev_collateral.
// The user's own lev_collateral_amount is already in user_token_in_ai too.
//
// 1. swap   lev_collateral + borrow → position token
// 2. add_collateral  deposit position token into omnipair
// 3. borrow          borrow repay_amount of lev_collateral
// 4. repay           send repay_amount to repay vault
#[allow(clippy::too_many_arguments)]
fn handle_open<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
    ra: &[AccountInfo<'info>],
    omnipair_program: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    user: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token0_in: bool,
    is_lev_collateral0: bool,
    token_in_mint: AccountInfo<'info>,
    token_out_mint: AccountInfo<'info>,
    user_token_in_ai: AccountInfo<'info>,
    user_token_out_ai: AccountInfo<'info>,
    swap_amount_in: u64,
    min_amount_out: u64,
    repay_amount: u64,
) -> Result<()> {
    // Record token_out balance before swap to compute exact delta
    let balance_before_swap = if token0_in {
        ctx.accounts.receiver_token1_account.amount
    } else {
        ctx.accounts.receiver_token0_account.amount
    };

    // 1. Swap lev_collateral → position token
    cpi_swap(
        omnipair_program, event_authority, ra, user, token_program,
        user_token_in_ai.clone(),
        user_token_out_ai.clone(),
        token_in_mint.clone(),
        token_out_mint.clone(),
        swap_amount_in,
        min_amount_out,
    )?;

    // Compute exact position token received (delta from swap)
    let amount_out = if token0_in {
        ctx.accounts.receiver_token1_account.reload()?;
        ctx.accounts.receiver_token1_account.amount
            .checked_sub(balance_before_swap)
            .ok_or(LeverageError::SwapFailed)?
    } else {
        ctx.accounts.receiver_token0_account.reload()?;
        ctx.accounts.receiver_token0_account.amount
            .checked_sub(balance_before_swap)
            .ok_or(LeverageError::SwapFailed)?
    };
    require!(amount_out > 0, LeverageError::SwapFailed);

    // Write position_size into the leverage position account via raw byte offset.
    // discriminator(8) + owner(32) + pair(32) + is_lev_collateral0(1) +
    // lev_collateral_amount(8) + multiplier_bps(8) = offset 89
    {
        const POSITION_SIZE_OFFSET: usize = 8 + 32 + 32 + 1 + 8 + 8;
        let mut data = ra[IDX_USER_LEV_POSITION].try_borrow_mut_data()?;
        data[POSITION_SIZE_OFFSET..POSITION_SIZE_OFFSET + 8]
            .copy_from_slice(&amount_out.to_le_bytes());
    }

    // 2. Add collateral
    cpi_adjust_collateral(
        omnipair_program, event_authority, ra, user, token_program,
        ra[IDX_COLLATERAL_VAULT].to_account_info(),
        user_token_out_ai.clone(),
        token_out_mint.clone(),
        b"global:add_collateral",
        amount_out,
    )?;

    // 3. Borrow repay_amount of lev_collateral
    cpi_adjust_debt(
        omnipair_program, event_authority, ra, user, token_program,
        ra[IDX_TOKEN_IN_VAULT].to_account_info(), // lev_collateral reserve vault
        user_token_in_ai.clone(),
        token_in_mint.clone(),
        b"global:borrow",
        repay_amount,
    )?;

    // 4. Repay flashloan
    let (repay_vault, repay_mint, repay_decimals) = if is_lev_collateral0 {
        (ctx.accounts.repay0_vault.to_account_info(), ctx.accounts.token0_mint.to_account_info(), ctx.accounts.token0_mint.decimals)
    } else {
        (ctx.accounts.repay1_vault.to_account_info(), ctx.accounts.token1_mint.to_account_info(), ctx.accounts.token1_mint.decimals)
    };
    anchor_spl::token::transfer_checked(
        CpiContext::new(
            token_program.clone(),
            TransferChecked {
                from:      user_token_in_ai,
                mint:      repay_mint,
                to:        repay_vault,
                authority: user.clone(),
            },
        ),
        repay_amount,
        repay_decimals,
    )
}

// ── CPI helpers ───────────────────────────────────────────────────────────────

/// CPI to omnipair's repay or borrow instruction (both use CommonAdjustDebt accounts).
/// Account order: pair, user_position, rate_model, futarchy, reserve_vault,
///                user_reserve_account, reserve_mint, user, token_program,
///                token_2022, system_program, event_authority, omnipair_program
#[allow(clippy::too_many_arguments)]
fn cpi_adjust_debt<'info>(
    omnipair_program: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    ra: &[AccountInfo<'info>],
    user: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    reserve_vault: AccountInfo<'info>,
    user_reserve_account: AccountInfo<'info>,
    reserve_mint: AccountInfo<'info>,
    discriminator_str: &[u8],
    amount: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(ra[IDX_PAIR].key(), false),
        AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
        AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
        AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
        AccountMeta::new(reserve_vault.key(), false),
        AccountMeta::new(user_reserve_account.key(), false),
        AccountMeta::new_readonly(reserve_mint.key(), false),
        AccountMeta::new(user.key(), true),
        AccountMeta::new_readonly(token_program.key(), false),
        AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
        AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ];
    let mut ix_data = hash(discriminator_str).to_bytes()[..8].to_vec();
    AdjustDebtArgs { amount }.serialize(&mut ix_data)?;
    invoke(
        &Instruction { program_id: omnipair_program.key(), accounts: metas, data: ix_data },
        &[
            ra[IDX_PAIR].to_account_info(),
            ra[IDX_USER_POSITION].to_account_info(),
            ra[IDX_RATE_MODEL].to_account_info(),
            ra[IDX_FUTARCHY].to_account_info(),
            reserve_vault,
            user_reserve_account,
            reserve_mint,
            user.clone(),
            token_program.clone(),
            ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
            ra[IDX_SYSTEM_PROGRAM].to_account_info(),
            event_authority.to_account_info(),
            omnipair_program.to_account_info(),
        ],
    )?;
    Ok(())
}

/// CPI to omnipair's add_collateral or remove_collateral instruction.
/// Account order: pair, user_position, rate_model, futarchy, collateral_vault,
///                user_collateral_account, collateral_mint, user, token_program,
///                token_2022, system_program, event_authority, omnipair_program
#[allow(clippy::too_many_arguments)]
fn cpi_adjust_collateral<'info>(
    omnipair_program: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    ra: &[AccountInfo<'info>],
    user: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    collateral_vault: AccountInfo<'info>,
    user_collateral_account: AccountInfo<'info>,
    collateral_mint: AccountInfo<'info>,
    discriminator_str: &[u8],
    amount: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(ra[IDX_PAIR].key(), false),
        AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
        AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
        AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
        AccountMeta::new(collateral_vault.key(), false),
        AccountMeta::new(user_collateral_account.key(), false),
        AccountMeta::new_readonly(collateral_mint.key(), false),
        AccountMeta::new(user.key(), true),
        AccountMeta::new_readonly(token_program.key(), false),
        AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
        AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ];
    let mut ix_data = hash(discriminator_str).to_bytes()[..8].to_vec();
    AdjustCollateralArgs { amount }.serialize(&mut ix_data)?;
    invoke(
        &Instruction { program_id: omnipair_program.key(), accounts: metas, data: ix_data },
        &[
            ra[IDX_PAIR].to_account_info(),
            ra[IDX_USER_POSITION].to_account_info(),
            ra[IDX_RATE_MODEL].to_account_info(),
            ra[IDX_FUTARCHY].to_account_info(),
            collateral_vault,
            user_collateral_account,
            collateral_mint,
            user.clone(),
            token_program.clone(),
            ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
            ra[IDX_SYSTEM_PROGRAM].to_account_info(),
            event_authority.to_account_info(),
            omnipair_program.to_account_info(),
        ],
    )?;
    Ok(())
}

/// CPI to omnipair's swap instruction.
/// Account order: pair, rate_model, futarchy, reserve_in, reserve_out,
///                user_in, user_out, mint_in, mint_out, user, token_program,
///                token_2022, event_authority, omnipair_program
#[allow(clippy::too_many_arguments)]
fn cpi_swap<'info>(
    omnipair_program: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    ra: &[AccountInfo<'info>],
    user: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    user_token_in: AccountInfo<'info>,
    user_token_out: AccountInfo<'info>,
    token_in_mint: AccountInfo<'info>,
    token_out_mint: AccountInfo<'info>,
    amount_in: u64,
    min_amount_out: u64,
) -> Result<()> {
    let metas = vec![
        AccountMeta::new(ra[IDX_PAIR].key(), false),
        AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
        AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
        AccountMeta::new(ra[IDX_TOKEN_IN_VAULT].key(), false),
        AccountMeta::new(ra[IDX_TOKEN_OUT_VAULT].key(), false),
        AccountMeta::new(user_token_in.key(), false),
        AccountMeta::new(user_token_out.key(), false),
        AccountMeta::new_readonly(token_in_mint.key(), false),
        AccountMeta::new_readonly(token_out_mint.key(), false),
        AccountMeta::new_readonly(user.key(), true),
        AccountMeta::new_readonly(token_program.key(), false),
        AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ];
    let mut ix_data = hash(b"global:swap").to_bytes()[..8].to_vec();
    SwapArgs { amount_in, min_amount_out }.serialize(&mut ix_data)?;
    invoke(
        &Instruction { program_id: omnipair_program.key(), accounts: metas, data: ix_data },
        &[
            ra[IDX_PAIR].to_account_info(),
            ra[IDX_RATE_MODEL].to_account_info(),
            ra[IDX_FUTARCHY].to_account_info(),
            ra[IDX_TOKEN_IN_VAULT].to_account_info(),
            ra[IDX_TOKEN_OUT_VAULT].to_account_info(),
            user_token_in,
            user_token_out,
            token_in_mint,
            token_out_mint,
            user.clone(),
            token_program.clone(),
            ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
            event_authority.to_account_info(),
            omnipair_program.to_account_info(),
        ],
    )?;
    Ok(())
}
