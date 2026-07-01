use crate::{
    constants::*,
    errors::ErrorCode,
    events::{
        AdjustDebtEvent, EventMetadata, SwapEvent, UserPositionLiquidatedEvent,
        UserPositionUpdatedEvent,
    },
    generate_gamm_pair_seeds,
    state::futarchy_authority::FutarchyAuthority,
    state::pair::Pair,
    state::rate_model::RateModel,
    state::user_position::{DebtDecreaseReason, UserPosition},
    utils::{
        math::ceil_div,
        token::{
            transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
        },
    },
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};
use std::cmp::min;

#[event_cpi]
#[derive(Accounts)]
pub struct Liquidate<'info> {
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
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            position_owner.key().as_ref()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,

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
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_collateral_vault_bump(&collateral_token_mint.key())
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = caller_token_account.mint == collateral_vault.mint,
    )]
    pub caller_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidVault
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = debt_token_mint.key() == pair.get_debt_token(&collateral_token_mint.key()) @ ErrorCode::InvalidMint
    )]
    pub debt_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            debt_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&debt_token_mint.key())
    )]
    pub debt_reserve_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = liquidator_debt_token_account.mint == debt_token_mint.key() @ ErrorCode::InvalidTokenAccount,
        constraint = liquidator_debt_token_account.owner == payer.key() @ ErrorCode::InvalidTokenAccount,
    )]
    pub liquidator_debt_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&collateral_token_mint.key())
    )]
    pub collateral_reserve_vault: Box<Account<'info, TokenAccount>>,

    /// CHECK: This is the owner of the position being liquidated.
    #[account(address = user_position.owner)]
    pub position_owner: AccountInfo<'info>,
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Liquidate<'info> {
    pub fn validate(&self) -> Result<()> {
        let user_position = &self.user_position;

        require!(
            user_position.is_initialized(),
            ErrorCode::UserPositionNotInitialized
        );

        // Check if user has enough debt
        match self.collateral_token_mint.key() == self.pair.token0 {
            true => require_gt!(user_position.debt1_shares, 0, ErrorCode::ZeroDebtAmount),
            false => require_gt!(user_position.debt0_shares, 0, ErrorCode::ZeroDebtAmount),
        }

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

    pub fn update_and_validate_liquidate(&mut self) -> Result<()> {
        self.update()?;
        self.validate()?;
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>) -> Result<()> {
        let Liquidate {
            collateral_vault,
            caller_token_account,
            collateral_token_mint,
            debt_token_mint,
            debt_reserve_vault,
            liquidator_debt_token_account,
            collateral_reserve_vault,
            position_owner,
            payer,
            user_position,
            token_program,
            token_2022_program,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;

        // Validate collateral vault and pool vault - already validated by Anchor seeds
        require_keys_eq!(
            collateral_vault.mint,
            collateral_token_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            debt_reserve_vault.mint,
            debt_token_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            debt_reserve_vault.owner,
            pair.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            collateral_reserve_vault.mint,
            collateral_token_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            collateral_reserve_vault.owner,
            pair.key(),
            ErrorCode::InvalidVault
        );

        let collateral_token = collateral_token_mint.key();
        let debt_token = debt_token_mint.key();
        require_keys_eq!(
            debt_token,
            pair.get_debt_token(&collateral_token),
            ErrorCode::InvalidMint
        );
        let is_collateral_token0 = collateral_token == pair.token0;
        let liquidation_cf_bps = user_position.get_liquidation_cf_bps(pair, &debt_token)?;
        let k0 = pair.k(); // k before liquidation

        // Compute debt
        let (
            user_debt,
            user_collateral,
            collateral_ema_nad,
            user_debt_shares,
            total_debt,
            total_debt_shares,
        ) = match is_collateral_token0 {
            true => (
                // collateral is token0, debt is token1
                user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
                user_position.collateral0,
                pair.ema_price0_nad(),
                user_position.debt1_shares,
                pair.total_debt1,
                pair.total_debt1_shares,
            ),
            false => (
                // collateral is token1, debt is token0
                user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
                user_position.collateral1,
                pair.ema_price1_nad(),
                user_position.debt0_shares,
                pair.total_debt0,
                pair.total_debt0_shares,
            ),
        };

        require!(collateral_ema_nad > 0, ErrorCode::InsufficientLiquidity);

        // Reference-price collateral value in debt-token units. This deliberately
        // avoids AMM depth/price-impact valuation so LP withdrawals do not move
        // liquidation eligibility through reserve depth alone.
        let collateral_value =
            collateral_value_at_reference_price(user_collateral, collateral_ema_nad)?;

        // Borrow limit = collateral_value * liquidation_cf
        let borrow_limit = collateral_value
            .checked_mul(liquidation_cf_bps as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        // Position is liquidatable if debt >= borrow_limit
        require_gte!(
            user_debt as u128,
            borrow_limit,
            ErrorCode::NotUndercollateralized
        );

        // If debt exceeds reference collateral value, close the position and
        // socialize only the uncovered portion after the liquidator repays the
        // maximum debt value supportable by the seized collateral.
        let is_insolvent = (user_debt as u128) > collateral_value;

        // Calculate shares to reduce first
        // For partial liquidation: ceil(user_debt_shares * CLOSE_FACTOR_BPS / BPS_DENOMINATOR)
        // For insolvent positions: all user debt shares
        let shares_to_reduce: u128 = match is_insolvent {
            true => user_debt_shares,
            false => {
                // ceiled division to avoid edge case where small shares never get fully written off
                let partial_shares = ceil_div(
                    user_debt_shares
                        .checked_mul(CLOSE_FACTOR_BPS as u128)
                        .ok_or(ErrorCode::DebtMathOverflow)?,
                    BPS_DENOMINATOR as u128,
                )
                .ok_or(ErrorCode::DebtMathOverflow)?;
                min(user_debt_shares, partial_shares) // clamped to user's shares
            }
        };

        // Use floor division for debt to ensure shares drain faster than debt.
        // This prevents orphaned user shares when total_debt hits 0 first (ceil/ceil problem).
        // Instead, total_shares hits 0 first, leaving orphaned debt which the sync reset safely clears.
        let debt_to_reduce: u64 = match total_debt_shares == 0 {
            true => 0,
            false => {
                let debt = shares_to_reduce
                    .checked_mul(total_debt as u128)
                    .ok_or(ErrorCode::DebtMathOverflow)?
                    .checked_div(total_debt_shares)
                    .ok_or(ErrorCode::DebtMathOverflow)?;
                min(user_debt, debt as u64) // clamped to user's debt
            }
        };

        let max_repay_from_collateral = max_debt_repayable_by_collateral(
            user_collateral,
            collateral_ema_nad,
            LIQUIDATION_PENALTY_BPS,
        )?;
        let repay_amount = match is_insolvent {
            true => min(debt_to_reduce, max_repay_from_collateral),
            false => debt_to_reduce,
        };
        validate_liquidation_progress(
            is_insolvent,
            shares_to_reduce,
            debt_to_reduce,
            repay_amount,
        )?;
        if repay_amount > 0 {
            require_gte!(
                liquidator_debt_token_account.amount,
                repay_amount,
                ErrorCode::InsufficientBalance
            );
        }

        // Base collateral covers the repaid debt at the EMA reference price.
        let collateral_base =
            collateral_amount_for_debt_at_reference_price(repay_amount, collateral_ema_nad, 0)?;

        // Total collateral seized includes the full liquidation penalty. In the
        // insolvent path all collateral is exhausted before any loss is socialized.
        let collateral_final = match is_insolvent {
            true => user_collateral,
            false => {
                let collateral_with_penalty = collateral_amount_for_debt_at_reference_price(
                    repay_amount,
                    collateral_ema_nad,
                    LIQUIDATION_PENALTY_BPS,
                )?;
                min(collateral_with_penalty, user_collateral)
            }
        };

        let collateral_token = pair.get_collateral_token(&debt_token);
        let collateral_amount_pre_liquidation = match collateral_token == pair.token0 {
            true => user_position.collateral0,
            false => user_position.collateral1,
        };

        let collateral_amount_post_liquidation = collateral_amount_pre_liquidation
            .checked_sub(collateral_final)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(
            &pair,
            &collateral_token,
            collateral_amount_post_liquidation,
        )?;

        // Liquidator receives repaid-debt collateral value plus the incentive.
        let collateral_to_liquidator = min(
            collateral_amount_for_debt_at_reference_price(
                repay_amount,
                collateral_ema_nad,
                LIQUIDATION_INCENTIVE_BPS,
            )?,
            collateral_final,
        );

        // Remaining collateral goes to reserves as the LP-side penalty.
        let collateral_to_reserves = collateral_final
            .checked_sub(collateral_to_liquidator)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let liquidation_bonus = collateral_to_liquidator.saturating_sub(collateral_base);

        let debt_token_program = match debt_token_mint.to_account_info().owner == token_program.key
        {
            true => token_program.to_account_info(),
            false => token_2022_program.to_account_info(),
        };
        let collateral_token_program =
            match collateral_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            };

        if repay_amount > 0 {
            transfer_from_user_to_vault(
                payer.to_account_info(),
                liquidator_debt_token_account.to_account_info(),
                debt_reserve_vault.to_account_info(),
                debt_token_mint.to_account_info(),
                debt_token_program,
                repay_amount,
                debt_token_mint.decimals,
            )?;
        }

        // Pass exact shares to avoid edge cases where floor division leaves residual shares.
        user_position.decrease_debt(
            pair,
            &debt_token,
            debt_to_reduce,
            DebtDecreaseReason::LiquidationRepayment {
                exact_shares: shares_to_reduce,
                cash_credit: repay_amount,
            },
        )?;
        user_position.set_liquidation_cf_for_debt_token(&debt_token, &pair, liquidation_cf_bps);

        // Transfer seized collateral to caller from collateral vault.
        if collateral_to_liquidator > 0 {
            transfer_from_vault_to_user(
                pair.to_account_info(),
                collateral_vault.to_account_info(),
                caller_token_account.to_account_info(),
                collateral_token_mint.to_account_info(),
                collateral_token_program.clone(),
                collateral_to_liquidator,
                collateral_token_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        // Transfer remaining collateral from collateral vault to reserve vault
        transfer_from_vault_to_vault(
            pair.to_account_info(),
            collateral_vault.to_account_info(),
            collateral_reserve_vault.to_account_info(),
            collateral_token_mint.to_account_info(),
            collateral_token_program,
            collateral_to_reserves,
            collateral_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Update user position collateral and pair reserves
        // Subtract the full seized amount from user position
        match is_collateral_token0 {
            true => {
                user_position.collateral0 = user_position
                    .collateral0
                    .checked_sub(collateral_final)
                    .unwrap();
                pair.total_collateral0 = pair
                    .total_collateral0
                    .checked_sub(collateral_final)
                    .unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve0 = pair.reserve0.checked_add(collateral_to_reserves).unwrap();
                pair.cash_reserve0 = pair.cash_reserve0.saturating_add(collateral_to_reserves);
            }
            false => {
                user_position.collateral1 = user_position
                    .collateral1
                    .checked_sub(collateral_final)
                    .unwrap();
                pair.total_collateral1 = pair
                    .total_collateral1
                    .checked_sub(collateral_final)
                    .unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve1 = pair.reserve1.checked_add(collateral_to_reserves).unwrap();
                pair.cash_reserve1 = pair.cash_reserve1.saturating_add(collateral_to_reserves);
            }
        }

        emit_cpi!(SwapEvent {
            metadata: EventMetadata::new(payer.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in: debt_token == pair.token0,
            amount_in: repay_amount,
            amount_out: collateral_to_liquidator,
            amount_in_after_fee: repay_amount,
            lp_fee: 0,
            protocol_fee: 0,
        });

        // Emit debt adjustment event (debt repaid and possibly socialized if insolvent)
        let (amount0, amount1) = if is_collateral_token0 {
            (0, -(debt_to_reduce as i64))
        } else {
            (-(debt_to_reduce as i64), 0)
        };
        emit_cpi!(AdjustDebtEvent {
            metadata: EventMetadata::new(position_owner.key(), pair.key()),
            amount0,
            amount1,
        });

        // Emit position updated event
        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(position_owner.key(), pair.key()),
            position: user_position.key(),
            collateral0: user_position.collateral0,
            collateral1: user_position.collateral1,
            debt0_shares: user_position.debt0_shares,
            debt1_shares: user_position.debt1_shares,
            collateral0_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token1),
            collateral1_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token0),
            collateral0_liquidation_cf_bps: user_position.collateral0_liquidation_cf_bps,
            collateral1_liquidation_cf_bps: user_position.collateral1_liquidation_cf_bps,
        });

        emit_cpi!(UserPositionLiquidatedEvent {
            metadata: EventMetadata::new(position_owner.key(), pair.key()),
            position: user_position.key(),
            liquidator: payer.key(),
            collateral0_liquidated: if is_collateral_token0 {
                collateral_final
            } else {
                0
            },
            collateral1_liquidated: if is_collateral_token0 {
                0
            } else {
                collateral_final
            },
            debt0_liquidated: if is_collateral_token0 {
                0
            } else {
                debt_to_reduce
            },
            debt1_liquidated: if is_collateral_token0 {
                debt_to_reduce
            } else {
                0
            },
            collateral_price: if is_collateral_token0 {
                pair.ema_price0_nad()
            } else {
                pair.ema_price1_nad()
            },
            shortfall: debt_to_reduce.saturating_sub(repay_amount) as u128,
            liquidation_bonus_applied: liquidation_bonus,
            k0: k0,
            k1: pair.k(),
        });

        Ok(())
    }
}

fn collateral_value_at_reference_price(collateral_amount: u64, price_nad: u64) -> Result<u128> {
    require!(price_nad > 0, ErrorCode::InsufficientLiquidity);
    (collateral_amount as u128)
        .checked_mul(price_nad as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::DebtMathOverflow.into())
}

fn collateral_amount_for_debt_at_reference_price(
    debt_amount: u64,
    price_nad: u64,
    penalty_bps: u16,
) -> Result<u64> {
    require!(price_nad > 0, ErrorCode::InsufficientLiquidity);
    let debt_with_penalty = ceil_div(
        (debt_amount as u128)
            .checked_mul((BPS_DENOMINATOR + penalty_bps) as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::DebtMathOverflow)?;
    let collateral_amount = ceil_div(
        debt_with_penalty
            .checked_mul(NAD as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?,
        price_nad as u128,
    )
    .ok_or(ErrorCode::DebtMathOverflow)?;
    u64::try_from(collateral_amount).map_err(|_| ErrorCode::DebtMathOverflow.into())
}

fn max_debt_repayable_by_collateral(
    collateral_amount: u64,
    price_nad: u64,
    penalty_bps: u16,
) -> Result<u64> {
    let collateral_value = collateral_value_at_reference_price(collateral_amount, price_nad)?;
    let repay_amount = collateral_value
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div((BPS_DENOMINATOR + penalty_bps) as u128))
        .ok_or(ErrorCode::DebtMathOverflow)?;
    u64::try_from(repay_amount).map_err(|_| ErrorCode::DebtMathOverflow.into())
}

fn validate_liquidation_progress(
    is_insolvent: bool,
    shares_to_reduce: u128,
    debt_to_reduce: u64,
    repay_amount: u64,
) -> Result<()> {
    require!(shares_to_reduce > 0, ErrorCode::ZeroDebtAmount);

    if is_insolvent {
        // Insolvent cleanup must still be able to clear bad debt when the
        // remaining collateral has zero repayable value after integer pricing.
        return Ok(());
    }

    require!(debt_to_reduce > 0, ErrorCode::ZeroDebtAmount);
    require!(repay_amount > 0, ErrorCode::InsufficientDebt);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_price_amounts_apply_liquidation_penalty() {
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD, 0).unwrap(),
            100
        );
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD, LIQUIDATION_INCENTIVE_BPS)
                .unwrap(),
            101
        );
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD, LIQUIDATION_PENALTY_BPS)
                .unwrap(),
            103
        );
    }

    #[test]
    fn reference_price_amounts_round_up_after_price_conversion() {
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD * 2, 0).unwrap(),
            50
        );
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD * 2, LIQUIDATION_INCENTIVE_BPS)
                .unwrap(),
            51
        );
        assert_eq!(
            collateral_amount_for_debt_at_reference_price(100, NAD * 2, LIQUIDATION_PENALTY_BPS)
                .unwrap(),
            52
        );
    }

    #[test]
    fn max_repayable_by_collateral_reserves_total_penalty() {
        assert_eq!(
            max_debt_repayable_by_collateral(103, NAD, LIQUIDATION_PENALTY_BPS).unwrap(),
            100
        );
    }

    #[test]
    fn zero_value_insolvent_collateral_can_be_cleaned_up() {
        assert_eq!(collateral_value_at_reference_price(1, 1).unwrap(), 0);
        assert_eq!(
            max_debt_repayable_by_collateral(1, 1, LIQUIDATION_PENALTY_BPS).unwrap(),
            0
        );
        assert!(validate_liquidation_progress(true, 1, 1, 0).is_ok());
    }

    #[test]
    fn insolvent_dust_shares_can_be_cleaned_up_without_nominal_debt() {
        assert!(validate_liquidation_progress(true, 1, 0, 0).is_ok());
    }

    #[test]
    fn solvent_zero_amount_liquidation_is_rejected() {
        let err = validate_liquidation_progress(false, 1, 0, 0).unwrap_err();
        assert_eq!(err, error!(ErrorCode::ZeroDebtAmount));

        let err = validate_liquidation_progress(false, 1, 1, 0).unwrap_err();
        assert_eq!(err, error!(ErrorCode::InsufficientDebt));
    }
}
