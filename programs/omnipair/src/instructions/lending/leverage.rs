use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::*,
    generate_gamm_pair_seeds,
    instructions::lending::common::AdjustDebtArgs,
    state::{
        DebtDecreaseReason, FutarchyAuthority, Pair, RateModel, UserLeveragePosition, UserPosition,
    },
    utils::{
        account::get_size_with_discriminator,
        gamm_math::CPCurve,
        math::ceil_div,
        token::{transfer_from_user_to_vault, transfer_from_vault_to_user},
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct OpenLeverageArgs {
    pub is_lev_collateral0: bool,
    pub lev_collateral_amount: u64,
    pub multiplier_bps: u64,
    pub max_slippage_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct CloseLeverageArgs {
    pub is_lev_collateral0: bool,
    pub min_collateral_out: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplyAmounts {
    pub swap_amount_in: u64,
    pub borrow_amount: u64,
    pub flashloan_fee: u64,
    pub repay_amount: u64,
    pub min_amount_out: u64,
}

pub fn compute_multiply_amounts(
    lev_collateral_amount: u64,
    multiplier_bps: u64,
    max_slippage_bps: u64,
    reserve_in: u64,
    reserve_out: u64,
) -> Result<MultiplyAmounts> {
    require!(lev_collateral_amount > 0, ErrorCode::AmountZero);
    require!(
        multiplier_bps > BPS_DENOMINATOR as u64,
        ErrorCode::InvalidArgument
    );
    require!(
        max_slippage_bps <= BPS_DENOMINATOR as u64,
        ErrorCode::InvalidArgument
    );
    require!(
        reserve_in > 0 && reserve_out > 0,
        ErrorCode::InsufficientLiquidity
    );

    let swap_amount_in: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;

    let borrow_amount = swap_amount_in
        .checked_sub(lev_collateral_amount)
        .ok_or(ErrorCode::Overflow)?;

    let flashloan_fee = ceil_div(
        (borrow_amount as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;

    let repay_amount = borrow_amount
        .checked_add(flashloan_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;

    let spot_out: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_mul(reserve_out as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(
            (reserve_in as u128)
                .checked_mul(BPS_DENOMINATOR as u128)
                .ok_or(ErrorCode::Overflow)?,
        )
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;

    let min_amount_out: u64 = ((spot_out as u128)
        .saturating_mul((BPS_DENOMINATOR as u128).saturating_sub(max_slippage_bps as u128))
        / BPS_DENOMINATOR as u128)
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;

    Ok(MultiplyAmounts {
        swap_amount_in,
        borrow_amount,
        flashloan_fee,
        repay_amount,
        min_amount_out,
    })
}

pub fn compute_close_repay_amounts(debt_amount: u64) -> Result<(u64, u64)> {
    require!(debt_amount > 0, ErrorCode::ZeroDebtAmount);

    let flashloan_fee = ceil_div(
        (debt_amount as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;

    let repay_amount = debt_amount
        .checked_add(flashloan_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;

    Ok((flashloan_fee, repay_amount))
}

#[inline]
pub fn leverage_swap_token0_is_input(is_lev_collateral0: bool, is_close: bool) -> bool {
    is_lev_collateral0 ^ is_close
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: OpenLeverageArgs)]
pub struct OpenLeverage<'info> {
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
        bump = pair.bump
    )]
    pub pair: Box<Account<'info, Pair>>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init_if_needed,
        payer = user,
        space = get_size_with_discriminator::<UserPosition>(),
        constraint = user_position.owner == Pubkey::default() || user_position.owner == user.key(),
        constraint = user_position.pair == Pubkey::default() || user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(
        init,
        payer = user,
        space = get_size_with_discriminator::<UserLeveragePosition>(),
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref(),
            &[args.is_lev_collateral0 as u8]
        ],
        bump,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_in_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_in_mint.key())
    )]
    pub token_in_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_out_mint.key())
    )]
    pub token_out_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump = pair.get_collateral_vault_bump(&token_out_mint.key())
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_in_account.mint == token_in_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_in_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_out_account.mint == token_out_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_out_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = token_in_mint.key() == pair.token0 || token_in_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_in_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = token_out_mint.key() == pair.token0 || token_out_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_out_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: CloseLeverageArgs)]
pub struct CloseLeverage<'info> {
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
        bump = pair.bump
    )]
    pub pair: Box<Account<'info, Pair>>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        mut,
        constraint = user_position.owner == user.key(),
        constraint = user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(
        mut,
        close = user,
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref(),
            &[args.is_lev_collateral0 as u8]
        ],
        bump = user_leverage_position.bump,
        constraint = user_leverage_position.owner == user.key(),
        constraint = user_leverage_position.pair == pair.key(),
        constraint = user_leverage_position.is_lev_collateral0 == args.is_lev_collateral0,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_in_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_in_mint.key())
    )]
    pub token_in_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_out_mint.key())
    )]
    pub token_out_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_in_mint.key().as_ref(),
        ],
        bump = pair.get_collateral_vault_bump(&token_in_mint.key())
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_in_account.mint == token_in_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_in_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_out_account.mint == token_out_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_out_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = token_in_mint.key() == pair.token0 || token_in_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_in_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = token_out_mint.key() == pair.token0 || token_out_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_out_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> OpenLeverage<'info> {
    pub fn handle_open_leverage(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: OpenLeverageArgs,
    ) -> Result<()> {
        let accounts = &mut ctx.accounts;
        let pair_key = accounts.pair.key();
        let lev_collateral_mint = if args.is_lev_collateral0 {
            accounts.pair.token0
        } else {
            accounts.pair.token1
        };
        let position_mint = accounts.pair.get_token_y(&lev_collateral_mint);

        require_keys_eq!(
            accounts.token_in_mint.key(),
            lev_collateral_mint,
            ErrorCode::InvalidMint
        );
        require_keys_eq!(
            accounts.token_out_mint.key(),
            position_mint,
            ErrorCode::InvalidMint
        );
        require!(
            !accounts
                .futarchy_authority
                .is_reduce_only(accounts.pair.reduce_only),
            ErrorCode::ReduceOnlyMode
        );
        require_gte!(
            accounts.user_token_in_account.amount,
            args.lev_collateral_amount,
            ErrorCode::InsufficientBalance
        );

        update_pair(
            &mut accounts.pair,
            &accounts.rate_model,
            &accounts.futarchy_authority,
            pair_key,
            accounts.event_authority.to_account_info(),
        )?;

        let reserve_in = if args.is_lev_collateral0 {
            accounts.pair.reserve0
        } else {
            accounts.pair.reserve1
        };
        let reserve_out = if args.is_lev_collateral0 {
            accounts.pair.reserve1
        } else {
            accounts.pair.reserve0
        };
        let existing_debt = current_debt(
            &accounts.user_position,
            &accounts.pair,
            &lev_collateral_mint,
        )?;
        let existing_position_collateral = if position_mint == accounts.pair.token0 {
            accounts.user_position.collateral0
        } else {
            accounts.user_position.collateral1
        };
        require!(
            existing_debt == 0 && existing_position_collateral == 0,
            ErrorCode::LeveragePositionNotIsolated
        );

        let amounts = compute_multiply_amounts(
            args.lev_collateral_amount,
            args.multiplier_bps,
            args.max_slippage_bps,
            reserve_in,
            reserve_out,
        )?;

        require_temp_loan_cash(
            &accounts.pair,
            args.is_lev_collateral0,
            amounts.borrow_amount,
        )?;

        transfer_pair_to_user(
            &accounts.pair,
            &accounts.token_in_vault,
            &accounts.user_token_in_account,
            &accounts.token_in_mint,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            amounts.borrow_amount,
        )?;
        accounts.user_token_in_account.reload()?;

        let balance_before_swap = accounts.user_token_out_account.amount;
        swap_internal(
            &mut accounts.pair,
            &accounts.futarchy_authority,
            &accounts.token_in_vault,
            &accounts.token_out_vault,
            &accounts.user_token_in_account,
            &accounts.user_token_out_account,
            &accounts.token_in_mint,
            &accounts.token_out_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
            amounts.swap_amount_in,
            amounts.min_amount_out,
        )?;
        accounts.user_token_out_account.reload()?;
        let position_size = accounts
            .user_token_out_account
            .amount
            .checked_sub(balance_before_swap)
            .ok_or(ErrorCode::OutputAmountOverflow)?;
        require!(position_size > 0, ErrorCode::InsufficientOutputAmount);

        add_collateral_internal(
            &mut accounts.pair,
            &mut accounts.user_position,
            Some(ctx.bumps.user_position),
            &accounts.collateral_vault,
            &accounts.user_token_out_account,
            &accounts.token_out_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
            position_size,
        )?;
        accounts.user_token_out_account.reload()?;

        borrow_internal(
            &mut accounts.pair,
            &accounts.futarchy_authority,
            &mut accounts.user_position,
            &accounts.token_in_vault,
            &accounts.user_token_in_account,
            &accounts.token_in_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
            amounts.repay_amount,
        )?;
        accounts.user_token_in_account.reload()?;
        let debt_shares = current_debt_shares(
            &accounts.user_position,
            &accounts.pair,
            &lev_collateral_mint,
        );

        settle_temp_loan(
            &mut accounts.pair,
            &accounts.token_in_vault,
            &accounts.user_token_in_account,
            &accounts.token_in_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            amounts.repay_amount,
            amounts.flashloan_fee,
        )?;

        accounts.user_leverage_position.initialize(
            accounts.user.key(),
            accounts.pair.key(),
            args.is_lev_collateral0,
            args.lev_collateral_amount,
            args.multiplier_bps,
            position_size,
            amounts.borrow_amount,
            debt_shares,
            Clock::get()?.unix_timestamp,
            ctx.bumps.user_leverage_position,
        );

        Ok(())
    }
}

impl<'info> CloseLeverage<'info> {
    pub fn handle_close_leverage(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: CloseLeverageArgs,
    ) -> Result<()> {
        let accounts = &mut ctx.accounts;
        let pair_key = accounts.pair.key();
        let lev_collateral_mint = if args.is_lev_collateral0 {
            accounts.pair.token0
        } else {
            accounts.pair.token1
        };
        let position_mint = accounts.pair.get_token_y(&lev_collateral_mint);

        require_keys_eq!(
            accounts.token_in_mint.key(),
            position_mint,
            ErrorCode::InvalidMint
        );
        require_keys_eq!(
            accounts.token_out_mint.key(),
            lev_collateral_mint,
            ErrorCode::InvalidMint
        );

        update_pair(
            &mut accounts.pair,
            &accounts.rate_model,
            &accounts.futarchy_authority,
            pair_key,
            accounts.event_authority.to_account_info(),
        )?;

        let position_collateral = if position_mint == accounts.pair.token0 {
            accounts.user_position.collateral0
        } else {
            accounts.user_position.collateral1
        };
        let debt_shares = current_debt_shares(
            &accounts.user_position,
            &accounts.pair,
            &lev_collateral_mint,
        );
        require!(
            position_collateral == accounts.user_leverage_position.position_size
                && debt_shares == accounts.user_leverage_position.debt_shares,
            ErrorCode::LeveragePositionNotIsolated
        );

        let debt_amount = current_debt(
            &accounts.user_position,
            &accounts.pair,
            &lev_collateral_mint,
        )?;
        let (flashloan_fee, repay_amount) = compute_close_repay_amounts(debt_amount)?;
        require_temp_loan_cash(&accounts.pair, args.is_lev_collateral0, debt_amount)?;

        transfer_pair_to_user(
            &accounts.pair,
            &accounts.token_out_vault,
            &accounts.user_token_out_account,
            &accounts.token_out_mint,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            debt_amount,
        )?;
        accounts.user_token_out_account.reload()?;

        repay_internal(
            &mut accounts.pair,
            &mut accounts.user_position,
            &accounts.token_out_vault,
            &accounts.user_token_out_account,
            &accounts.token_out_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
            AdjustDebtArgs {
                amount: debt_amount,
            },
        )?;
        accounts.user_token_out_account.reload()?;

        let balance_before_remove = accounts.user_token_in_account.amount;
        remove_all_collateral_internal(
            &mut accounts.pair,
            &mut accounts.user_position,
            &accounts.collateral_vault,
            &accounts.user_token_in_account,
            &accounts.token_in_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
        )?;
        accounts.user_token_in_account.reload()?;
        let amount_in_swap = accounts
            .user_token_in_account
            .amount
            .checked_sub(balance_before_remove)
            .ok_or(ErrorCode::OutputAmountOverflow)?;
        require!(amount_in_swap > 0, ErrorCode::InsufficientOutputAmount);

        swap_internal(
            &mut accounts.pair,
            &accounts.futarchy_authority,
            &accounts.token_in_vault,
            &accounts.token_out_vault,
            &accounts.user_token_in_account,
            &accounts.user_token_out_account,
            &accounts.token_in_mint,
            &accounts.token_out_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            accounts.event_authority.to_account_info(),
            amount_in_swap,
            args.min_collateral_out,
        )?;
        accounts.user_token_out_account.reload()?;

        settle_temp_loan(
            &mut accounts.pair,
            &accounts.token_out_vault,
            &accounts.user_token_out_account,
            &accounts.token_out_mint,
            &accounts.user,
            &accounts.token_program.to_account_info(),
            &accounts.token_2022_program.to_account_info(),
            repay_amount,
            flashloan_fee,
        )
    }
}

fn update_pair<'info>(
    pair: &mut Account<'info, Pair>,
    rate_model: &Account<'info, RateModel>,
    futarchy_authority: &FutarchyAuthority,
    pair_key: Pubkey,
    event_authority: AccountInfo<'info>,
) -> Result<()> {
    pair.update(
        rate_model,
        futarchy_authority,
        pair_key,
        Some(event_authority),
    )
}

fn require_temp_loan_cash(pair: &Pair, is_token0: bool, amount: u64) -> Result<()> {
    match is_token0 {
        true => require_gte!(pair.cash_reserve0, amount, ErrorCode::BorrowExceedsReserve),
        false => require_gte!(pair.cash_reserve1, amount, ErrorCode::BorrowExceedsReserve),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transfer_pair_to_user<'info>(
    pair: &Account<'info, Pair>,
    reserve_vault: &Account<'info, TokenAccount>,
    user_token_account: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let pair_seeds = generate_gamm_pair_seeds!(pair);
    let signer_seeds = &[&pair_seeds[..]];
    let program =
        token_program_for_mint(&mint.to_account_info(), token_program, token_2022_program);

    transfer_from_vault_to_user(
        pair.to_account_info(),
        reserve_vault.to_account_info(),
        user_token_account.to_account_info(),
        mint.to_account_info(),
        program,
        amount,
        mint.decimals,
        signer_seeds,
    )
}

#[allow(clippy::too_many_arguments)]
fn swap_internal<'info>(
    pair: &mut Account<'info, Pair>,
    futarchy_authority: &FutarchyAuthority,
    token_in_vault: &Account<'info, TokenAccount>,
    token_out_vault: &Account<'info, TokenAccount>,
    user_token_in_account: &Account<'info, TokenAccount>,
    user_token_out_account: &Account<'info, TokenAccount>,
    token_in_mint: &Account<'info, Mint>,
    token_out_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    event_authority: AccountInfo<'info>,
    amount_in: u64,
    min_amount_out: u64,
) -> Result<()> {
    require!(amount_in > 0, ErrorCode::AmountZero);
    require_gte!(
        user_token_in_account.amount,
        amount_in,
        ErrorCode::InsufficientBalance
    );
    require_keys_neq!(
        token_in_vault.key(),
        token_out_vault.key(),
        ErrorCode::InvalidVaultSameAccount
    );

    let last_k = pair.k();
    let is_token0_in = user_token_in_account.mint == pair.token0;

    if is_token0_in {
        require_keys_eq!(
            token_in_vault.mint,
            pair.token0,
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            token_out_vault.mint,
            pair.token1,
            ErrorCode::InvalidTokenAccount
        );
    } else {
        require_keys_eq!(
            token_in_vault.mint,
            pair.token1,
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            token_out_vault.mint,
            pair.token0,
            ErrorCode::InvalidTokenAccount
        );
    }

    let swap_fee = ceil_div(
        (amount_in as u128)
            .checked_mul(pair.swap_fee_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;

    let futarchy_fee = ceil_div(
        (swap_fee as u128)
            .checked_mul(futarchy_authority.revenue_share.swap_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;

    let amount_in_after_swap_fee = amount_in
        .checked_sub(swap_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;
    let reserve_in = if is_token0_in {
        pair.reserve0
    } else {
        pair.reserve1
    };
    let reserve_out = if is_token0_in {
        pair.reserve1
    } else {
        pair.reserve0
    };
    let amount_out =
        CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_swap_fee)?;
    let amount_in_with_lp_fee = amount_in
        .checked_sub(futarchy_fee)
        .ok_or(ErrorCode::Overflow)?;
    let new_reserve_in = reserve_in
        .checked_add(amount_in_with_lp_fee)
        .ok_or(ErrorCode::Overflow)?;
    let new_reserve_out = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::Overflow)?;

    require_gte!(amount_out, min_amount_out, ErrorCode::SlippageExceeded);
    match is_token0_in {
        true => require_gte!(
            pair.cash_reserve1,
            amount_out,
            ErrorCode::InsufficientCashReserve1
        ),
        false => require_gte!(
            pair.cash_reserve0,
            amount_out,
            ErrorCode::InsufficientCashReserve0
        ),
    }

    match is_token0_in {
        true => {
            pair.reserve0 = new_reserve_in;
            pair.reserve1 = new_reserve_out;
            pair.cash_reserve0 = pair.cash_reserve0.saturating_add(amount_in_with_lp_fee);
            pair.cash_reserve1 = pair.cash_reserve1.saturating_sub(amount_out);
        }
        false => {
            pair.reserve1 = new_reserve_in;
            pair.reserve0 = new_reserve_out;
            pair.cash_reserve1 = pair.cash_reserve1.saturating_add(amount_in_with_lp_fee);
            pair.cash_reserve0 = pair.cash_reserve0.saturating_sub(amount_out);
        }
    }

    require_gte!(pair.k(), last_k, ErrorCode::BrokenInvariant);

    transfer_from_user_to_vault(
        user.to_account_info(),
        user_token_in_account.to_account_info(),
        token_in_vault.to_account_info(),
        token_in_mint.to_account_info(),
        token_program_for_mint(
            &token_in_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        amount_in,
        token_in_mint.decimals,
    )?;

    let pair_seeds = generate_gamm_pair_seeds!(pair);
    let signer_seeds = &[&pair_seeds[..]];
    transfer_from_vault_to_user(
        pair.to_account_info(),
        token_out_vault.to_account_info(),
        user_token_out_account.to_account_info(),
        token_out_mint.to_account_info(),
        token_program_for_mint(
            &token_out_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        amount_out,
        token_out_mint.decimals,
        signer_seeds,
    )?;

    let lp_fee = swap_fee.checked_sub(futarchy_fee).unwrap_or(0);
    emit_swap_event(
        event_authority,
        SwapEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in,
            amount_in,
            amount_out,
            amount_in_after_fee: amount_in_after_swap_fee,
            lp_fee,
            protocol_fee: futarchy_fee,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn add_collateral_internal<'info>(
    pair: &mut Account<'info, Pair>,
    user_position: &mut Account<'info, UserPosition>,
    user_position_bump: Option<u8>,
    collateral_vault: &Account<'info, TokenAccount>,
    user_collateral_token_account: &Account<'info, TokenAccount>,
    collateral_token_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    event_authority: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::AmountZero);
    require_gte!(
        user_collateral_token_account.amount,
        amount,
        ErrorCode::InsufficientBalanceForCollateral
    );

    if !user_position.is_initialized() {
        user_position.initialize(
            user.key(),
            pair.key(),
            user_position_bump.ok_or(ErrorCode::UserPositionNotInitialized)?,
        )?;
        emit_user_position_created_event(
            event_authority.clone(),
            UserPositionCreatedEvent {
                metadata: EventMetadata::new(user.key(), pair.key()),
                position: user_position.key(),
            },
        )?;
    }

    let is_collateral_token0 = user_collateral_token_account.mint == pair.token0;
    transfer_from_user_to_vault(
        user.to_account_info(),
        user_collateral_token_account.to_account_info(),
        collateral_vault.to_account_info(),
        collateral_token_mint.to_account_info(),
        token_program_for_mint(
            &collateral_token_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        amount,
        collateral_token_mint.decimals,
    )?;

    match is_collateral_token0 {
        true => {
            pair.total_collateral0 = pair
                .total_collateral0
                .checked_add(amount)
                .ok_or(ErrorCode::Overflow)?;
            user_position.collateral0 = user_position
                .collateral0
                .checked_add(amount)
                .ok_or(ErrorCode::Overflow)?;
        }
        false => {
            pair.total_collateral1 = pair
                .total_collateral1
                .checked_add(amount)
                .ok_or(ErrorCode::Overflow)?;
            user_position.collateral1 = user_position
                .collateral1
                .checked_add(amount)
                .ok_or(ErrorCode::Overflow)?;
        }
    }

    let (amount0, amount1) = if is_collateral_token0 {
        (amount as i64, 0)
    } else {
        (0, amount as i64)
    };
    emit_adjust_collateral_event(
        event_authority.clone(),
        AdjustCollateralEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        },
    )?;
    emit_user_position_updated_event(
        event_authority,
        user_position_updated_event(
            user.key(),
            pair.key(),
            user_position.key(),
            pair,
            user_position,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn borrow_internal<'info>(
    pair: &mut Account<'info, Pair>,
    futarchy_authority: &FutarchyAuthority,
    user_position: &mut Account<'info, UserPosition>,
    reserve_vault: &Account<'info, TokenAccount>,
    user_reserve_token_account: &Account<'info, TokenAccount>,
    reserve_token_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    event_authority: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    require!(
        !futarchy_authority.is_reduce_only(pair.reduce_only),
        ErrorCode::ReduceOnlyMode
    );
    require!(amount > 0, ErrorCode::AmountZero);

    let is_token0 = user_reserve_token_account.mint == pair.token0;
    let user_debt = match is_token0 {
        true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
        false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
    };
    let collateral_token = pair.get_collateral_token(&reserve_token_mint.key());
    let collateral_amount = match collateral_token == pair.token0 {
        true => user_position.collateral0,
        false => user_position.collateral1,
    };
    let (borrow_limit, _, liquidation_cf_bps) =
        pair.get_max_debt_and_cf_bps_for_collateral(pair, &collateral_token, collateral_amount)?;
    let new_debt = user_debt
        .checked_add(amount)
        .ok_or(ErrorCode::DebtMathOverflow)?;

    require_gte!(borrow_limit, new_debt, ErrorCode::BorrowingPowerExceeded);
    match is_token0 {
        true => require_gte!(
            pair.cash_reserve0,
            amount,
            ErrorCode::InsufficientCashReserve0
        ),
        false => require_gte!(
            pair.cash_reserve1,
            amount,
            ErrorCode::InsufficientCashReserve1
        ),
    }

    transfer_pair_to_user(
        pair,
        reserve_vault,
        user_reserve_token_account,
        reserve_token_mint,
        token_program,
        token_2022_program,
        amount,
    )?;

    user_position.increase_debt(pair, &reserve_token_mint.key(), amount)?;
    user_position.set_liquidation_cf_for_debt_token(
        &reserve_token_mint.key(),
        pair,
        liquidation_cf_bps,
    );

    let (amount0, amount1) = if is_token0 {
        (amount as i64, 0)
    } else {
        (0, amount as i64)
    };
    emit_adjust_debt_event(
        event_authority.clone(),
        AdjustDebtEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        },
    )?;
    emit_user_position_updated_event(
        event_authority,
        user_position_updated_event(
            user.key(),
            pair.key(),
            user_position.key(),
            pair,
            user_position,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn repay_internal<'info>(
    pair: &mut Account<'info, Pair>,
    user_position: &mut Account<'info, UserPosition>,
    reserve_vault: &Account<'info, TokenAccount>,
    user_reserve_token_account: &Account<'info, TokenAccount>,
    reserve_token_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    event_authority: AccountInfo<'info>,
    args: AdjustDebtArgs,
) -> Result<()> {
    require!(args.amount > 0, ErrorCode::AmountZero);
    let is_token0 = user_reserve_token_account.mint == pair.token0;
    let user_total_debt = match is_token0 {
        true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
        false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
    };
    require_gt!(user_total_debt, 0, ErrorCode::ZeroDebtAmount);
    require_gte!(user_total_debt, args.amount, ErrorCode::InsufficientDebt);
    require_gte!(
        user_reserve_token_account.amount,
        args.amount,
        ErrorCode::InsufficientBalance
    );

    transfer_from_user_to_vault(
        user.to_account_info(),
        user_reserve_token_account.to_account_info(),
        reserve_vault.to_account_info(),
        reserve_token_mint.to_account_info(),
        token_program_for_mint(
            &reserve_token_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        args.amount,
        reserve_token_mint.decimals,
    )?;

    user_position.decrease_debt(
        pair,
        &reserve_token_mint.key(),
        args.amount,
        DebtDecreaseReason::Repayment,
    )?;

    let (amount0, amount1) = if is_token0 {
        (-(args.amount as i64), 0)
    } else {
        (0, -(args.amount as i64))
    };
    emit_adjust_debt_event(
        event_authority.clone(),
        AdjustDebtEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        },
    )?;
    emit_user_position_updated_event(
        event_authority,
        user_position_updated_event(
            user.key(),
            pair.key(),
            user_position.key(),
            pair,
            user_position,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn remove_all_collateral_internal<'info>(
    pair: &mut Account<'info, Pair>,
    user_position: &mut Account<'info, UserPosition>,
    collateral_vault: &Account<'info, TokenAccount>,
    user_collateral_token_account: &Account<'info, TokenAccount>,
    collateral_token_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    event_authority: AccountInfo<'info>,
) -> Result<()> {
    let is_token0 = user_collateral_token_account.mint == pair.token0;
    let opposite_debt = match is_token0 {
        true => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        false => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
    };
    require!(opposite_debt == 0, ErrorCode::BorrowingPowerExceeded);

    let withdraw_amount = if is_token0 {
        user_position.collateral0
    } else {
        user_position.collateral1
    };
    require!(withdraw_amount > 0, ErrorCode::AmountZero);

    let pair_seeds = generate_gamm_pair_seeds!(pair);
    let signer_seeds = &[&pair_seeds[..]];
    transfer_from_vault_to_user(
        pair.to_account_info(),
        collateral_vault.to_account_info(),
        user_collateral_token_account.to_account_info(),
        collateral_token_mint.to_account_info(),
        token_program_for_mint(
            &collateral_token_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        withdraw_amount,
        collateral_token_mint.decimals,
        signer_seeds,
    )?;

    match is_token0 {
        true => {
            pair.total_collateral0 = pair
                .total_collateral0
                .checked_sub(withdraw_amount)
                .ok_or(ErrorCode::Overflow)?;
            user_position.collateral0 = user_position
                .collateral0
                .checked_sub(withdraw_amount)
                .ok_or(ErrorCode::Overflow)?;
        }
        false => {
            pair.total_collateral1 = pair
                .total_collateral1
                .checked_sub(withdraw_amount)
                .ok_or(ErrorCode::Overflow)?;
            user_position.collateral1 = user_position
                .collateral1
                .checked_sub(withdraw_amount)
                .ok_or(ErrorCode::Overflow)?;
        }
    }

    let collateral_token = if is_token0 { pair.token0 } else { pair.token1 };
    let debt_token = if is_token0 { pair.token1 } else { pair.token0 };
    let collateral_amount = if is_token0 {
        user_position.collateral0
    } else {
        user_position.collateral1
    };
    let (_, _, liquidation_cf_bps) =
        pair.get_max_debt_and_cf_bps_for_collateral(pair, &collateral_token, collateral_amount)?;
    user_position.set_liquidation_cf_for_debt_token(&debt_token, pair, liquidation_cf_bps);

    let (amount0, amount1) = if is_token0 {
        (-(withdraw_amount as i64), 0)
    } else {
        (0, -(withdraw_amount as i64))
    };
    emit_adjust_collateral_event(
        event_authority.clone(),
        AdjustCollateralEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        },
    )?;
    emit_user_position_updated_event(
        event_authority,
        user_position_updated_event(
            user.key(),
            pair.key(),
            user_position.key(),
            pair,
            user_position,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn settle_temp_loan<'info>(
    pair: &mut Account<'info, Pair>,
    reserve_vault: &Account<'info, TokenAccount>,
    user_reserve_token_account: &Account<'info, TokenAccount>,
    reserve_token_mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    repay_amount: u64,
    flashloan_fee: u64,
) -> Result<()> {
    require_gte!(
        user_reserve_token_account.amount,
        repay_amount,
        ErrorCode::InsufficientBalance
    );

    transfer_from_user_to_vault(
        user.to_account_info(),
        user_reserve_token_account.to_account_info(),
        reserve_vault.to_account_info(),
        reserve_token_mint.to_account_info(),
        token_program_for_mint(
            &reserve_token_mint.to_account_info(),
            token_program,
            token_2022_program,
        ),
        repay_amount,
        reserve_token_mint.decimals,
    )?;

    if reserve_token_mint.key() == pair.token0 {
        pair.reserve0 = pair.reserve0.saturating_add(flashloan_fee);
        pair.cash_reserve0 = pair.cash_reserve0.saturating_add(flashloan_fee);
    } else {
        pair.reserve1 = pair.reserve1.saturating_add(flashloan_fee);
        pair.cash_reserve1 = pair.cash_reserve1.saturating_add(flashloan_fee);
    }
    Ok(())
}

fn current_debt(user_position: &UserPosition, pair: &Pair, debt_token: &Pubkey) -> Result<u64> {
    if *debt_token == pair.token0 {
        user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)
    } else {
        user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)
    }
}

fn current_debt_shares(user_position: &UserPosition, pair: &Pair, debt_token: &Pubkey) -> u128 {
    if *debt_token == pair.token0 {
        user_position.debt0_shares
    } else {
        user_position.debt1_shares
    }
}

fn token_program_for_mint<'info>(
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
) -> AccountInfo<'info> {
    if mint.owner == token_program.key {
        token_program.clone()
    } else {
        token_2022_program.clone()
    }
}

fn user_position_updated_event(
    user: Pubkey,
    pair_key: Pubkey,
    position_key: Pubkey,
    pair: &Pair,
    user_position: &UserPosition,
) -> UserPositionUpdatedEvent {
    UserPositionUpdatedEvent {
        metadata: EventMetadata::new(user, pair_key),
        position: position_key,
        collateral0: user_position.collateral0,
        collateral1: user_position.collateral1,
        debt0_shares: user_position.debt0_shares,
        debt1_shares: user_position.debt1_shares,
        collateral0_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token1),
        collateral1_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token0),
        collateral0_liquidation_cf_bps: user_position.collateral0_liquidation_cf_bps,
        collateral1_liquidation_cf_bps: user_position.collateral1_liquidation_cf_bps,
    }
}

macro_rules! event_cpi_ctx {
    ($event_authority:expr) => {{
        let (_, event_authority_bump) =
            Pubkey::find_program_address(&[b"__event_authority"], &crate::ID);

        struct EventCpiAccounts<'a> {
            event_authority: AccountInfo<'a>,
        }
        struct EventCpiBumps {
            event_authority: u8,
        }
        struct EventCpiContext<'a> {
            accounts: EventCpiAccounts<'a>,
            bumps: EventCpiBumps,
        }

        EventCpiContext {
            accounts: EventCpiAccounts {
                event_authority: $event_authority,
            },
            bumps: EventCpiBumps {
                event_authority: event_authority_bump,
            },
        }
    }};
}

fn emit_swap_event<'info>(event_authority: AccountInfo<'info>, event: SwapEvent) -> Result<()> {
    let ctx = event_cpi_ctx!(event_authority);
    emit_cpi!(event);
    Ok(())
}

fn emit_adjust_collateral_event<'info>(
    event_authority: AccountInfo<'info>,
    event: AdjustCollateralEvent,
) -> Result<()> {
    let ctx = event_cpi_ctx!(event_authority);
    emit_cpi!(event);
    Ok(())
}

fn emit_adjust_debt_event<'info>(
    event_authority: AccountInfo<'info>,
    event: AdjustDebtEvent,
) -> Result<()> {
    let ctx = event_cpi_ctx!(event_authority);
    emit_cpi!(event);
    Ok(())
}

fn emit_user_position_created_event<'info>(
    event_authority: AccountInfo<'info>,
    event: UserPositionCreatedEvent,
) -> Result<()> {
    let ctx = event_cpi_ctx!(event_authority);
    emit_cpi!(event);
    Ok(())
}

fn emit_user_position_updated_event<'info>(
    event_authority: AccountInfo<'info>,
    event: UserPositionUpdatedEvent,
) -> Result<()> {
    let ctx = event_cpi_ctx!(event_authority);
    emit_cpi!(event);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiply_2x_borrow_and_fee() {
        let m = compute_multiply_amounts(1_000, 20_000, 0, 1_000_000, 1_000_000).unwrap();
        assert_eq!(m.swap_amount_in, 2_000);
        assert_eq!(m.borrow_amount, 1_000);
        assert_eq!(m.flashloan_fee, 1);
        assert_eq!(m.repay_amount, 1_001);
        assert_eq!(m.min_amount_out, 2_000);
    }

    #[test]
    fn multiply_symmetric_reserves_spot_matches_constant_product_intuition() {
        let m = compute_multiply_amounts(100, 15_000, 0, 10_000, 10_000).unwrap();
        assert_eq!(m.swap_amount_in, 150);
        assert_eq!(m.borrow_amount, 50);
        assert_eq!(m.min_amount_out, 150);
    }

    #[test]
    fn multiply_slippage_clamps_min_out() {
        let no_slip = compute_multiply_amounts(100, 20_000, 0, 1_000, 2_000).unwrap();
        let slip_1pct = compute_multiply_amounts(100, 20_000, 100, 1_000, 2_000).unwrap();
        assert_eq!(
            slip_1pct.min_amount_out,
            no_slip.min_amount_out * 9_900 / 10_000
        );
    }

    #[test]
    fn multiply_rejects_zero_collateral() {
        assert!(compute_multiply_amounts(0, 20_000, 0, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_multiplier_at_1x() {
        assert!(compute_multiply_amounts(100, 10_000, 0, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_slippage_over_100pct() {
        assert!(compute_multiply_amounts(100, 20_000, 10_001, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_empty_reserves() {
        assert!(compute_multiply_amounts(100, 20_000, 0, 0, 1).is_err());
    }

    #[test]
    fn close_repay_rounds_fee_up() {
        let (fee, repay) = compute_close_repay_amounts(1).unwrap();
        assert_eq!(fee, 1);
        assert_eq!(repay, 2);
    }

    #[test]
    fn close_repay_zero_debt_errors() {
        assert!(compute_close_repay_amounts(0).is_err());
    }

    #[test]
    fn leverage_token0_in_truth_table() {
        assert!(leverage_swap_token0_is_input(true, false));
        assert!(!leverage_swap_token0_is_input(false, false));
        assert!(!leverage_swap_token0_is_input(true, true));
        assert!(leverage_swap_token0_is_input(false, true));
    }
}
