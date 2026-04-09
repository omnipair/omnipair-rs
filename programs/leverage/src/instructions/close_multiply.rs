use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use omnipair::state::{Pair, UserPosition, RateModel, FutarchyAuthority};
use crate::{
    constants::*,
    errors::LeverageError,
    instruction_math::compute_close_repay_amounts,
    state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX},
    types::InternalCallbackData,
    utils::{FlashloanAccounts, validate_remaining_accounts, invoke_flashloan_raw},
};

#[derive(Accounts)]
#[instruction(is_lev_collateral0: bool)]
pub struct CloseMultiply<'info> {
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub pair: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub rate_model: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub futarchy_authority: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve1_vault: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    pub token0_mint: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub token1_mint: UncheckedAccount<'info>,

    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay0_vault: UncheckedAccount<'info>,
    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay1_vault: UncheckedAccount<'info>,

    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    /// CHECK: must be this program's own ID
    #[account(address = crate::ID)]
    pub receiver_program: UncheckedAccount<'info>,

    /// Leverage position to close. Rent is returned to user.
    #[account(
        mut,
        close = user,
        seeds = [LEVERAGE_POSITION_SEED_PREFIX, pair.key().as_ref(), user.key().as_ref(), &[is_lev_collateral0 as u8]],
        bump = user_leverage_position.bump,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: SPL Token-2022 program
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    // remaining_accounts[0..11]: see constants.rs for layout
}

/// Closes a leveraged position and returns margin + PnL to the user.
///
/// Flashloans the outstanding debt (read live from omnipair), repays the borrow,
/// withdraws all collateral, swaps back to lev_collateral, repays the flashloan.
/// The remainder in the user's receiver account is their margin + PnL net of the
/// flashloan fee. Closes the UserLeveragePosition PDA and returns rent to user.
///
/// - `is_lev_collateral0`: must match the direction used when opening
/// - `min_collateral_out`: minimum lev_collateral token to receive after swapping back
pub fn handle<'info>(
    ctx: Context<'_, '_, 'info, 'info, CloseMultiply<'info>>,
    is_lev_collateral0: bool,
    min_collateral_out: u64,
) -> Result<()> {
    let ra = ctx.remaining_accounts;
    require!(ra.len() >= 12, LeverageError::MissingRemainingAccounts);

    // For close: TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve
    validate_remaining_accounts(
        ra,
        &ctx.accounts.pair.to_account_info(),
        &ctx.accounts.rate_model.to_account_info(),
        &ctx.accounts.futarchy_authority.to_account_info(),
        ctx.accounts.reserve0_vault.key(),
        ctx.accounts.reserve1_vault.key(),
        is_lev_collateral0,
        true, // close: TOKEN_IN is position token
        ctx.accounts.user_leverage_position.key(),
    )?;

    // ── update pair to accrue current-slot interest before reading debt ─
    // Without this, close_multiply snapshots a stale total_debt while the
    // flashloan's update_and_validate() accrues interest in the same tx,
    // causing the callback repay to demand more than the receiver holds.
    let rate_model = Account::<RateModel>::try_from(&ra[IDX_RATE_MODEL])?;
    let futarchy = FutarchyAuthority::try_deserialize(
        &mut &**ra[IDX_FUTARCHY].try_borrow_data()?
    )?;
    let pair_key = ra[IDX_PAIR].key();
    {
        let mut pair_buf = ra[IDX_PAIR].try_borrow_mut_data()?;
        let mut pair = Pair::try_deserialize(&mut pair_buf.as_ref())?;
        pair.update(&rate_model, &futarchy, pair_key, Some(ra[IDX_EVENT_AUTHORITY].clone()))?;
        let mut writer: &mut [u8] = &mut pair_buf;
        pair.try_serialize(&mut writer)?;
    }

    // ── read current debt from omnipair state ──────────────────────────
    let pair_data = Pair::try_deserialize(&mut &**ra[IDX_PAIR].try_borrow_data()?)?;
    let user_pos  = UserPosition::try_deserialize(&mut &**ra[IDX_USER_POSITION].try_borrow_data()?)?;

    let debt_amount = if is_lev_collateral0 {
        user_pos.calculate_debt0(pair_data.total_debt0, pair_data.total_debt0_shares)?
    } else {
        user_pos.calculate_debt1(pair_data.total_debt1, pair_data.total_debt1_shares)?
    };
    let (_flashloan_fee, repay_amount) = compute_close_repay_amounts(debt_amount)?;

    // ── invoke flashloan ───────────────────────────────────────────────
    let (amount0, amount1) = if is_lev_collateral0 {
        (debt_amount, 0u64)
    } else {
        (0u64, debt_amount)
    };

    invoke_flashloan_raw(
        &FlashloanAccounts {
            pair:                   ctx.accounts.pair.to_account_info(),
            rate_model:             ctx.accounts.rate_model.to_account_info(),
            futarchy_authority:     ctx.accounts.futarchy_authority.to_account_info(),
            reserve0_vault:         ctx.accounts.reserve0_vault.to_account_info(),
            reserve1_vault:         ctx.accounts.reserve1_vault.to_account_info(),
            token0_mint:            ctx.accounts.token0_mint.to_account_info(),
            token1_mint:            ctx.accounts.token1_mint.to_account_info(),
            repay0_vault:           ctx.accounts.repay0_vault.to_account_info(),
            repay1_vault:           ctx.accounts.repay1_vault.to_account_info(),
            receiver_token0_account: ctx.accounts.receiver_token0_account.to_account_info(),
            receiver_token1_account: ctx.accounts.receiver_token1_account.to_account_info(),
            receiver_program:       ctx.accounts.receiver_program.to_account_info(),
            user:                   ctx.accounts.user.to_account_info(),
            token_program:          ctx.accounts.token_program.to_account_info(),
            token_2022_program:     ctx.accounts.token_2022_program.to_account_info(),
            system_program:         ctx.accounts.system_program.to_account_info(),
            user_leverage_position: ctx.accounts.user_leverage_position.to_account_info(),
        },
        ra,
        amount0,
        amount1,
        InternalCallbackData {
            is_close: true,
            is_lev_collateral0,
            swap_amount_in: 0, // determined at runtime in callback
            min_amount_out: min_collateral_out,
            repay_amount,
        },
    )
    // Anchor closes user_leverage_position and returns rent to user after this returns.
}
