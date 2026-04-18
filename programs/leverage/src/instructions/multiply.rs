use crate::{
    constants::*,
    errors::LeverageError,
    instruction_math::compute_multiply_amounts,
    state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX},
    utils::{invoke_open_leverage_raw, validate_remaining_accounts, NativeLeverageAccounts},
};
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use omnipair::{
    state::{Pair, UserPosition},
    OpenLeverageArgs,
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

    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

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

    /// CHECK: PDA signer used to prove this CPI came from the leverage program.
    #[account(
        seeds = [LEVERAGE_AUTHORITY_SEED_PREFIX],
        bump,
    )]
    pub leverage_authority: UncheckedAccount<'info>,

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
/// Executes the leverage path through a native Omnipair instruction, avoiding
/// any flashloan callback reentry.
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

    let pair_data = Pair::try_deserialize(&mut &**ra[IDX_PAIR].try_borrow_data()?)?;
    let (reserve_in, reserve_out) = if is_lev_collateral0 {
        (pair_data.reserve0, pair_data.reserve1)
    } else {
        (pair_data.reserve1, pair_data.reserve0)
    };

    let amounts = compute_multiply_amounts(
        lev_collateral_amount,
        multiplier_bps,
        max_slippage_bps,
        reserve_in,
        reserve_out,
    )?;
    let borrow_amount = amounts.borrow_amount;

    let starting_position_size =
        position_token_collateral_or_zero(&ra[IDX_USER_POSITION], is_lev_collateral0)?;
    let (user_token_in_account, user_token_out_account, token_in_mint, token_out_mint) =
        if is_lev_collateral0 {
            (
                ctx.accounts.receiver_token0_account.to_account_info(),
                ctx.accounts.receiver_token1_account.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
            )
        } else {
            (
                ctx.accounts.receiver_token1_account.to_account_info(),
                ctx.accounts.receiver_token0_account.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
            )
        };

    invoke_open_leverage_raw(
        &NativeLeverageAccounts {
            pair: ctx.accounts.pair.to_account_info(),
            rate_model: ctx.accounts.rate_model.to_account_info(),
            futarchy_authority: ctx.accounts.futarchy_authority.to_account_info(),
            user_position: ra[IDX_USER_POSITION].to_account_info(),
            token_in_vault: ra[IDX_TOKEN_IN_VAULT].to_account_info(),
            token_out_vault: ra[IDX_TOKEN_OUT_VAULT].to_account_info(),
            collateral_vault: ra[IDX_COLLATERAL_VAULT].to_account_info(),
            user_token_in_account,
            user_token_out_account,
            token_in_mint,
            token_out_mint,
            user: ctx.accounts.user.to_account_info(),
            leverage_authority: ctx.accounts.leverage_authority.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            token_2022_program: ctx.accounts.token_2022_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        ra,
        OpenLeverageArgs {
            is_lev_collateral0,
            lev_collateral_amount,
            multiplier_bps,
            max_slippage_bps,
        },
    )?;

    let ending_position_size =
        read_position_token_collateral(&ra[IDX_USER_POSITION], is_lev_collateral0)?;
    let position_size = ending_position_size
        .checked_sub(starting_position_size)
        .ok_or(LeverageError::Overflow)?;
    require!(position_size > 0, LeverageError::SwapFailed);

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
    ctx.accounts.user_leverage_position.position_size = position_size;

    Ok(())
}

fn position_token_collateral_or_zero<'info>(
    user_position_ai: &AccountInfo<'info>,
    is_lev_collateral0: bool,
) -> Result<u64> {
    if user_position_ai.owner != &omnipair::ID {
        return Ok(0);
    }

    let data = user_position_ai.try_borrow_data()?;
    if data.len() < 8 || data.iter().all(|byte| *byte == 0) {
        return Ok(0);
    }

    match UserPosition::try_deserialize(&mut &data[..]) {
        Ok(user_position) => Ok(position_token_collateral(
            &user_position,
            is_lev_collateral0,
        )),
        Err(_) => Ok(0),
    }
}

fn read_position_token_collateral<'info>(
    user_position_ai: &AccountInfo<'info>,
    is_lev_collateral0: bool,
) -> Result<u64> {
    let user_position = UserPosition::try_deserialize(&mut &**user_position_ai.try_borrow_data()?)?;
    Ok(position_token_collateral(
        &user_position,
        is_lev_collateral0,
    ))
}

fn position_token_collateral(user_position: &UserPosition, is_lev_collateral0: bool) -> u64 {
    if is_lev_collateral0 {
        user_position.collateral1
    } else {
        user_position.collateral0
    }
}
