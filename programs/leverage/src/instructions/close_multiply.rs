use crate::{
    constants::*,
    errors::LeverageError,
    state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX},
    utils::{invoke_close_leverage_raw, validate_remaining_accounts, NativeLeverageAccounts},
};
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use omnipair::CloseLeverageArgs;

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

    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

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
/// Executes the close path through a native Omnipair instruction, repaying debt,
/// withdrawing collateral, swapping back to lev_collateral, and settling the
/// internal temporary loan without a flashloan callback.
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

    require_keys_eq!(
        ctx.accounts.user_leverage_position.owner,
        ctx.accounts.user.key(),
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ctx.accounts.user_leverage_position.pair,
        ctx.accounts.pair.key(),
        LeverageError::RemainingAccountMismatch
    );
    require!(
        ctx.accounts.user_leverage_position.is_lev_collateral0 == is_lev_collateral0,
        LeverageError::RemainingAccountMismatch
    );

    let (user_token_in_account, user_token_out_account, token_in_mint, token_out_mint) =
        if is_lev_collateral0 {
            (
                ctx.accounts.receiver_token1_account.to_account_info(),
                ctx.accounts.receiver_token0_account.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
            )
        } else {
            (
                ctx.accounts.receiver_token0_account.to_account_info(),
                ctx.accounts.receiver_token1_account.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
            )
        };

    invoke_close_leverage_raw(
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
            token_program: ctx.accounts.token_program.to_account_info(),
            token_2022_program: ctx.accounts.token_2022_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        ra,
        CloseLeverageArgs {
            is_lev_collateral0,
            min_collateral_out,
        },
    )
    // Anchor closes user_leverage_position and returns rent to user after this returns.
}
