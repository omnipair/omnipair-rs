use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use omnipair::{state::Pair, ceil_div};
use crate::{
    constants::*,
    errors::LeverageError,
    state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX},
    types::InternalCallbackData,
    utils::{FlashloanAccounts, validate_remaining_accounts, invoke_flashloan_raw},
};

#[derive(Accounts)]
#[instruction(is_lev_collateral0: bool)]
pub struct Multiply<'info> {
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

    /// Leverage position PDA for this (pair, user, side).
    /// Fails if already exists — close the position first.
    #[account(
        init,
        payer = user,
        space = 8 + UserLeveragePosition::INIT_SPACE,
        seeds = [LEVERAGE_POSITION_SEED_PREFIX, pair.key().as_ref(), user.key().as_ref(), &[is_lev_collateral0 as u8]],
        bump,
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

/// Opens a leveraged position.
///
/// Flashloans the borrow portion from omnipair, swaps everything into the
/// position token, deposits as collateral, borrows to repay — all atomic.
///
/// - `is_lev_collateral0`: true = long token0 (token0 is lev_collateral, token1 is position token)
/// - `multiplier_bps`: leverage in BPS (20_000 = 2×); must be > 10_000
/// - `max_slippage_bps`: max deviation from spot price (10_000 = no check)
pub fn handle<'info>(
    ctx: Context<'_, '_, '_, 'info, Multiply<'info>>,
    is_lev_collateral0: bool,
    lev_collateral_amount: u64,
    multiplier_bps: u64,
    max_slippage_bps: u64,
) -> Result<()> {
    require!(lev_collateral_amount > 0, LeverageError::AmountZero);
    require!(multiplier_bps > BPS_DENOMINATOR, LeverageError::MultiplierTooLow);
    require!(max_slippage_bps <= BPS_DENOMINATOR, LeverageError::InvalidSlippage);

    let ra = ctx.remaining_accounts;
    require!(ra.len() >= 12, LeverageError::MissingRemainingAccounts);

    validate_remaining_accounts(
        ra,
        &ctx.accounts.pair.to_account_info(),
        &ctx.accounts.rate_model.to_account_info(),
        &ctx.accounts.futarchy_authority.to_account_info(),
        ctx.accounts.reserve0_vault.key(),
        ctx.accounts.reserve1_vault.key(),
        is_lev_collateral0,
        false, // open: TOKEN_IN is lev_collateral
        ctx.accounts.user_leverage_position.key(),
    )?;

    // ── compute amounts ────────────────────────────────────────────────
    let swap_amount_in: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128).ok_or(LeverageError::Overflow)?
        .checked_div(BPS_DENOMINATOR as u128).ok_or(LeverageError::Overflow)?
        .try_into().map_err(|_| LeverageError::Overflow)?;

    let borrow_amount = swap_amount_in
        .checked_sub(lev_collateral_amount).ok_or(LeverageError::Overflow)?;

    let flashloan_fee = ceil_div(
        (borrow_amount as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128).ok_or(LeverageError::Overflow)?,
        BPS_DENOMINATOR as u128,
    ).ok_or(LeverageError::Overflow)? as u64;

    let repay_amount = borrow_amount
        .checked_add(flashloan_fee).ok_or(LeverageError::Overflow)?;

    // ── slippage: spot-price floor ─────────────────────────────────────
    let pair_data = Pair::try_deserialize(&mut &**ra[IDX_PAIR].try_borrow_data()?)?;
    let (reserve_in, reserve_out) = if is_lev_collateral0 {
        (pair_data.reserve0, pair_data.reserve1)
    } else {
        (pair_data.reserve1, pair_data.reserve0)
    };
    require!(reserve_in > 0 && reserve_out > 0, LeverageError::InsufficientLiquidity);

    let spot_out: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128).ok_or(LeverageError::Overflow)?
        .checked_mul(reserve_out as u128).ok_or(LeverageError::Overflow)?
        .checked_div(
            (reserve_in as u128)
                .checked_mul(BPS_DENOMINATOR as u128).ok_or(LeverageError::Overflow)?
        ).ok_or(LeverageError::Overflow)?
        .try_into().map_err(|_| LeverageError::Overflow)?;

    let min_amount_out: u64 = ((spot_out as u128)
        .saturating_mul((BPS_DENOMINATOR as u128).saturating_sub(max_slippage_bps as u128))
        / BPS_DENOMINATOR as u128)
        .try_into().map_err(|_| LeverageError::Overflow)?;

    // ── init user leverage position ────────────────────────────────────
    ctx.accounts.user_leverage_position.initialize(
        ctx.accounts.user.key(),
        ctx.accounts.pair.key(),
        is_lev_collateral0,
        lev_collateral_amount,
        multiplier_bps,
        borrow_amount,
        Clock::get()?.unix_timestamp,
        ctx.bumps.user_leverage_position,
    );

    // ── invoke flashloan ───────────────────────────────────────────────
    let (amount0, amount1) = if is_lev_collateral0 {
        (borrow_amount, 0u64)
    } else {
        (0u64, borrow_amount)
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
            is_close: false,
            is_lev_collateral0,
            swap_amount_in,
            min_amount_out,
            repay_amount,
        },
    )
}
