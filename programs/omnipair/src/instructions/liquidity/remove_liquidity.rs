use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::constants::*;
use crate::errors::ErrorCode;
use crate::events::{BurnEvent, EventMetadata, UserLiquidityPositionUpdatedEvent};
use crate::generate_gamm_pair_seeds;
use crate::state::{futarchy_authority::FutarchyAuthority, pair::Pair, rate_model::RateModel};
use crate::utils::gamm_math::{construct_virtual_reserves_at_pessimistic_price, CPCurve};
use crate::utils::liquidity_delta_circuit_breaker::{
    require_no_same_tx_add_liquidity, require_top_level_liquidity_delta_ix,
    LiquidityDeltaInstruction,
};
use crate::utils::math::ceil_div;
use crate::utils::token::{token_burn, transfer_from_vault_to_user};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemoveLiquidityArgs {
    pub liquidity_in: u64,
    pub min_amount0_out: u64,
    pub min_amount1_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref(),
            pair.params_hash.as_ref()
        ],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

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
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref(),
        ],
        bump = pair.vault_bumps.reserve0
    )]
    pub reserve0_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1_account: Box<Account<'info, TokenAccount>>,

    #[account(
        address = pair.token0 @ ErrorCode::InvalidMint
    )]
    pub token0_mint: Box<Account<'info, Mint>>,

    #[account(
        address = pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token1_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = pair.lp_mint @ ErrorCode::InvalidMint,
    )]
    pub lp_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        associated_token::mint = lp_mint,
        associated_token::authority = user,
        payer = user,
        token::token_program = token_program,
    )]
    pub user_lp_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Instructions sysvar used by the liquidity delta circuit breaker.
    #[account(address = sysvar::instructions::ID @ ErrorCode::InvalidInstructionsSysvar)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

impl<'info> RemoveLiquidity<'info> {
    fn validate_remove(&self, args: &RemoveLiquidityArgs) -> Result<()> {
        require_top_level_liquidity_delta_ix(
            &self.pair.key(),
            &self.instructions_sysvar.to_account_info(),
            LiquidityDeltaInstruction::RemoveLiquidity,
        )?;
        require_no_same_tx_add_liquidity(
            &self.pair.key(),
            &self.instructions_sysvar.to_account_info(),
        )?;

        require!(args.liquidity_in > 0, ErrorCode::AmountZero);
        require!(
            args.liquidity_in <= self.pair.total_supply,
            ErrorCode::InsufficientLiquidity
        );
        require!(
            self.user_lp_token_account.amount >= args.liquidity_in,
            ErrorCode::InsufficientBalance
        );

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;
        Ok(())
    }

    pub fn update_and_validate_remove(&mut self, args: &RemoveLiquidityArgs) -> Result<()> {
        self.update()?;
        self.validate_remove(args)?;
        Ok(())
    }

    pub fn handle_remove(ctx: Context<Self>, args: RemoveLiquidityArgs) -> Result<()> {
        let RemoveLiquidity {
            pair,
            user_lp_token_account,
            reserve0_vault,
            reserve1_vault,
            user_token0_account,
            user_token1_account,
            lp_mint,
            token_program,
            token_2022_program,
            token0_mint,
            token1_mint,
            ..
        } = ctx.accounts;

        // Calculate amounts to remove (before fee)
        let total_supply = pair.total_supply;
        let amount0_gross: u64 = (args.liquidity_in as u128)
            .checked_mul(pair.reserve0 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let amount1_gross: u64 = (args.liquidity_in as u128)
            .checked_mul(pair.reserve1 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        // Apply withdrawal fee (1%) - fee remains in reserves for remaining LPs
        let fee0 = ceil_div(
            (amount0_gross as u128)
                .checked_mul(LIQUIDITY_WITHDRAWAL_FEE_BPS as u128)
                .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::FeeMathOverflow)? as u64;
        let fee1 = ceil_div(
            (amount1_gross as u128)
                .checked_mul(LIQUIDITY_WITHDRAWAL_FEE_BPS as u128)
                .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let amount0_out = amount0_gross
            .checked_sub(fee0)
            .ok_or(ErrorCode::LiquidityMathOverflow)?;
        let amount1_out = amount1_gross
            .checked_sub(fee1)
            .ok_or(ErrorCode::LiquidityMathOverflow)?;

        // Check if amounts meet minimum (slippage protection)
        require!(
            amount0_out >= args.min_amount0_out,
            ErrorCode::SlippageExceeded
        );
        require!(
            amount1_out >= args.min_amount1_out,
            ErrorCode::SlippageExceeded
        );

        // Ensure sufficient cash reserves: (internally accounted instead of relying on token account balance for deciding liquidity availability)
        // - Token account balances may include protocol fees and external donation, allowing them
        //   to be higher than the virtual reserves (r_virtual).
        // - If the invariant r_cash + r_debt = r_virtual is broken, the pool's solvency
        //   assumption (r_virtual >= r_debt) may also be violated.
        require_gte!(
            pair.cash_reserve0,
            amount0_out,
            ErrorCode::InsufficientCashReserve0
        );
        require_gte!(
            pair.cash_reserve1,
            amount1_out,
            ErrorCode::InsufficientCashReserve1
        );

        let post_reserve0 = pair
            .reserve0
            .checked_sub(amount0_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        let post_reserve1 = pair
            .reserve1
            .checked_sub(amount1_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        validate_post_withdraw_debt_coverage(pair, post_reserve0, post_reserve1)?;

        // Transfer tokens from pool to user
        transfer_from_vault_to_user(
            pair.to_account_info(),
            reserve0_vault.to_account_info(),
            user_token0_account.to_account_info(),
            token0_mint.to_account_info(),
            match token0_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount0_out,
            token0_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        transfer_from_vault_to_user(
            pair.to_account_info(),
            reserve1_vault.to_account_info(),
            user_token1_account.to_account_info(),
            token1_mint.to_account_info(),
            match token1_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount1_out,
            token1_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Reload vault accounts to get updated balances after transfers
        reserve0_vault.reload()?;
        reserve1_vault.reload()?;

        // Burn LP tokens from user
        token_burn(
            ctx.accounts.user.to_account_info(),
            token_program.to_account_info(),
            lp_mint.to_account_info(),
            user_lp_token_account.to_account_info(),
            args.liquidity_in,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Update reserves
        pair.reserve0 = post_reserve0;
        pair.reserve1 = post_reserve1;
        pair.total_supply = pair
            .total_supply
            .checked_sub(args.liquidity_in)
            .ok_or(ErrorCode::SupplyUnderflow)?;

        // Update cash reserves
        pair.cash_reserve0 = pair
            .cash_reserve0
            .checked_sub(amount0_out)
            .ok_or(ErrorCode::CashReserveUnderflow)?;
        pair.cash_reserve1 = pair
            .cash_reserve1
            .checked_sub(amount1_out)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        // Reload LP token account to get updated balance after burn
        user_lp_token_account.reload()?;
        let user_lp_balance = user_lp_token_account.amount;

        // Calculate user's token amounts from LP balance (same formula as add_liquidity)
        let user_token0_amount = (user_lp_balance as u128)
            .checked_mul(pair.reserve0 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let user_token1_amount = (user_lp_balance as u128)
            .checked_mul(pair.reserve1 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        // Emit event
        emit_cpi!(BurnEvent {
            metadata: EventMetadata::new(ctx.accounts.user.key(), pair.key()),
            amount0: amount0_out,
            amount1: amount1_out,
            liquidity: args.liquidity_in,
        });

        emit_cpi!(UserLiquidityPositionUpdatedEvent {
            metadata: EventMetadata::new(ctx.accounts.user.key(), pair.key()),
            token0_amount: user_token0_amount,
            token1_amount: user_token1_amount,
            lp_amount: user_lp_balance,
            cash_reserve0: pair.cash_reserve0,
            cash_reserve1: pair.cash_reserve1,
            token0_mint: pair.token0,
            token1_mint: pair.token1,
            lp_mint: lp_mint.key(),
        });

        Ok(())
    }
}

fn validate_post_withdraw_debt_coverage(
    pair: &Pair,
    post_reserve0: u64,
    post_reserve1: u64,
) -> Result<()> {
    validate_post_withdraw_debt_coverage_with_prices(
        post_reserve0,
        post_reserve1,
        pair.total_debt0,
        pair.total_debt1,
        pair.ema_price0_nad(),
        pair.directional_ema_price0_nad(),
        pair.ema_price1_nad(),
        pair.directional_ema_price1_nad(),
    )
}

fn validate_post_withdraw_debt_coverage_with_prices(
    post_reserve0: u64,
    post_reserve1: u64,
    total_debt0: u64,
    total_debt1: u64,
    token0_ema_price_nad: u64,
    token0_directional_ema_price_nad: u64,
    token1_ema_price_nad: u64,
    token1_directional_ema_price_nad: u64,
) -> Result<()> {
    let required_token1_for_debt0 = required_collateral_with_impact(
        total_debt0,
        post_reserve1,
        post_reserve0,
        token1_ema_price_nad,
        token1_directional_ema_price_nad,
    )?;
    let required_token0_for_debt1 = required_collateral_with_impact(
        total_debt1,
        post_reserve0,
        post_reserve1,
        token0_ema_price_nad,
        token0_directional_ema_price_nad,
    )?;

    require!(
        (post_reserve1 as u128) >= with_debt_coverage_buffer(required_token1_for_debt0)?,
        ErrorCode::InsufficientPostWithdrawDebtCoverage
    );
    require!(
        (post_reserve0 as u128) >= with_debt_coverage_buffer(required_token0_for_debt1)?,
        ErrorCode::InsufficientPostWithdrawDebtCoverage
    );

    Ok(())
}

fn required_collateral_with_impact(
    debt_amount: u64,
    collateral_spot_reserve: u64,
    debt_spot_reserve: u64,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
) -> Result<u64> {
    if debt_amount == 0 {
        return Ok(0);
    }

    require!(
        collateral_ema_price_nad > 0 && collateral_directional_ema_price_nad > 0,
        ErrorCode::InsufficientPostWithdrawDebtCoverage
    );

    let (collateral_ema_reserve, debt_ema_reserve) =
        construct_virtual_reserves_at_pessimistic_price(
            collateral_spot_reserve,
            debt_spot_reserve,
            collateral_ema_price_nad,
            collateral_directional_ema_price_nad,
        )?;

    require!(
        collateral_ema_reserve > 0 && debt_ema_reserve > debt_amount,
        ErrorCode::InsufficientPostWithdrawDebtCoverage
    );

    CPCurve::calculate_amount_in(collateral_ema_reserve, debt_ema_reserve, debt_amount)
}

fn with_debt_coverage_buffer(amount: u64) -> Result<u128> {
    ceil_div(
        (amount as u128)
            .checked_mul(POST_WITHDRAW_DEBT_COVERAGE_BPS as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::DebtMathOverflow.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquidity_delta_withdrawal_solvency_passes_with_coverage_buffer() {
        validate_post_withdraw_debt_coverage_with_prices(
            1_000, 1_000, 100, 100, NAD, NAD, NAD, NAD,
        )
        .unwrap();
    }

    #[test]
    fn liquidity_delta_withdrawal_solvency_fails_without_coverage_buffer() {
        let err = validate_post_withdraw_debt_coverage_with_prices(
            1_000, 1_000, 900, 0, NAD, NAD, NAD, NAD,
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientPostWithdrawDebtCoverage));
    }

    #[test]
    fn liquidity_delta_withdrawal_solvency_fails_with_zero_pessimistic_price() {
        let err = validate_post_withdraw_debt_coverage_with_prices(
            1_000, 1_000, 100, 0, NAD, NAD, NAD, 0,
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientPostWithdrawDebtCoverage));
    }

    #[test]
    fn liquidity_delta_withdrawal_solvency_accounts_for_fee_remaining_in_reserves() {
        let gross = 100_u64;
        let fee = ceil_div(
            (gross as u128) * (LIQUIDITY_WITHDRAWAL_FEE_BPS as u128),
            BPS_DENOMINATOR as u128,
        )
        .unwrap() as u64;
        let amount_out = gross - fee;

        assert_eq!(fee, 1);
        assert_eq!(amount_out, 99);
        assert_eq!(1_000_u64 - amount_out, 901);
    }
}
