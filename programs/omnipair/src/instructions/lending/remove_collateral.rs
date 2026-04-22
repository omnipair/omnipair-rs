use anchor_lang::prelude::*;
use crate::{
    constants::PAIR_SEED_PREFIX,
    errors::ErrorCode,
    events::{AdjustCollateralEvent, EventMetadata, UserPositionUpdatedEvent},
    utils::token::transfer_from_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{CommonAdjustCollateral, AdjustCollateralArgs},
};

impl<'info> CommonAdjustCollateral<'info> {
    pub fn validate_remove(&self, args: &AdjustCollateralArgs) -> Result<()> {
        let AdjustCollateralArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);

        let collateral_token = self.user_collateral_token_account.mint;
        let is_collateral_token0 = collateral_token == self.pair.token0;
        let user_collateral = match is_collateral_token0 {
            true => self.user_position.collateral0,
            false => self.user_position.collateral1,
        };

        // Calculate current debt
        let debt = match is_collateral_token0 {
            true => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
            false => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
        };

        // Check reduce-only mode: if active, user must have zero debt to remove collateral
        if self.futarchy_authority.is_reduce_only(self.pair.reduce_only) {
            require!(debt == 0, ErrorCode::ReduceOnlyHasDebt);
        }

        let withdraw_amount = if *amount == u64::MAX && debt == 0 {
            user_collateral
        } else {
            *amount
        };
        require!(withdraw_amount > 0, ErrorCode::AmountZero);

        require_gte!(
            user_collateral,
            withdraw_amount,
            ErrorCode::InsufficientBalanceForCollateral
        );

        // If the user has debt, validate the exact post-withdraw position.
        if debt > 0 {
            let remaining_collateral = user_collateral
                .checked_sub(withdraw_amount)
                .ok_or(ErrorCode::Overflow)?;
            let collateral_token = if is_collateral_token0 { self.pair.token0 } else { self.pair.token1 };
            let (post_withdraw_borrow_limit, _, _) = self.pair.get_max_debt_and_cf_bps_for_collateral(
                &self.pair,
                &collateral_token,
                remaining_collateral,
            )?;
            require_gte!(
                post_withdraw_borrow_limit,
                debt,
                ErrorCode::BorrowingPowerExceeded
            );
        }
        
        Ok(())
    }

    pub fn update_and_validate_remove(&mut self, args: &AdjustCollateralArgs) -> Result<()> {
        self.update()?;
        self.validate_remove(args)?;
        Ok(())
    }

    pub fn handle_remove_collateral(ctx: Context<Self>, args: AdjustCollateralArgs) -> Result<()> {
        let CommonAdjustCollateral {
            pair,
            collateral_vault,
            user_collateral_token_account,
            collateral_token_mint,
            token_program,
            token_2022_program,
            user,
            user_position,
            ..
        } = ctx.accounts;

        let is_token0 = user_collateral_token_account.mint == pair.token0;
        let user_collateral = match is_token0 {
            true => user_position.collateral0,
            false => user_position.collateral1,
        };
        // Calculate current debt
        let debt = match is_token0 {
            true => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
            false => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
        };
        let withdraw_amount = if args.amount == u64::MAX && debt == 0 {
            user_collateral
        } else {
            args.amount
        };
        require!(withdraw_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            user_collateral,
            withdraw_amount,
            ErrorCode::InsufficientBalanceForCollateral
        );

        transfer_from_vault_to_user(
            pair.to_account_info(),
            collateral_vault.to_account_info(),
            user_collateral_token_account.to_account_info(),
            collateral_token_mint.to_account_info(),
            match collateral_vault.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            withdraw_amount,
            collateral_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Transfer tokens from vault to user
        match is_token0 {
            true => {
                pair.total_collateral0 = pair.total_collateral0.checked_sub(withdraw_amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_sub(withdraw_amount).unwrap();
            },
            false => {
                pair.total_collateral1 = pair.total_collateral1.checked_sub(withdraw_amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_sub(withdraw_amount).unwrap();
            }
        }

        let collateral_token = if is_token0 { pair.token0 } else { pair.token1 };
        let debt_token = if is_token0 { pair.token1 } else { pair.token0 };
        let collateral_amount = if is_token0 {
            user_position.collateral0
        } else {
            user_position.collateral1
        };
        let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount)?;
        user_position.set_liquidation_cf_for_debt_token(&debt_token, &pair, liquidation_cf_bps);

        // Emit collateral adjustment event
        let (amount0, amount1) = match is_token0 {
            true => (-(withdraw_amount as i64), 0),
            false => (0, -(withdraw_amount as i64)),
        };
        
        emit_cpi!(AdjustCollateralEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        });
        
        // Emit position updated event
        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::*;
    use crate::utils::gamm_math::{
        construct_virtual_reserves_at_pessimistic_price, pessimistic_max_debt, CPCurve,
    };
    use crate::utils::math::ceil_div;

    fn simulate_resolve_remove_collateral_amount(user_collateral: u64, debt: u64, amount: u64) -> u64 {
        if amount == u64::MAX && debt == 0 {
            user_collateral
        } else {
            amount
        }
    }

    fn simulate_linear_max_withdrawable(
        user_collateral: u64,
        debt: u64,
        total_debt: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        ema_price: u64,
        directional_ema_price: u64,
    ) -> u64 {
        let (_, max_cf_bps, _) = pessimistic_max_debt(
            user_collateral,
            ema_price,
            directional_ema_price,
            collateral_reserve,
            debt_reserve,
            total_debt,
            None,
        )
        .unwrap();
        let min_collateral_value = ceil_div(
            (debt as u128) * (BPS_DENOMINATOR as u128),
            max_cf_bps as u128,
        )
        .unwrap();
        let min_collateral = ceil_div(
            min_collateral_value * (NAD as u128),
            ema_price as u128,
        )
        .unwrap();
        let min_collateral_u64 = u64::try_from(min_collateral).unwrap_or(u64::MAX);

        user_collateral.saturating_sub(min_collateral_u64)
    }

    fn simulate_impact_inverse_max_withdrawable(
        user_collateral: u64,
        debt: u64,
        total_debt: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        ema_price: u64,
        directional_ema_price: u64,
    ) -> u64 {
        let (_, max_cf_bps, _) = pessimistic_max_debt(
            user_collateral,
            ema_price,
            directional_ema_price,
            collateral_reserve,
            debt_reserve,
            total_debt,
            None,
        )
        .unwrap();
        let min_collateral_value = ceil_div(
            (debt as u128) * (BPS_DENOMINATOR as u128),
            max_cf_bps as u128,
        )
        .unwrap();
        let (collateral_ema_reserve, debt_ema_reserve) =
            construct_virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            )
            .unwrap();
        let min_collateral = CPCurve::calculate_amount_in(
            collateral_ema_reserve,
            debt_ema_reserve,
            u64::try_from(min_collateral_value).unwrap_or(u64::MAX),
        )
        .unwrap();

        user_collateral.saturating_sub(min_collateral)
    }

    fn post_withdraw_borrow_limit(
        user_collateral: u64,
        total_debt: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        ema_price: u64,
        directional_ema_price: u64,
    ) -> u64 {
        let (borrow_limit, _, _) = pessimistic_max_debt(
            user_collateral,
            ema_price,
            directional_ema_price,
            collateral_reserve,
            debt_reserve,
            total_debt,
            None,
        )
        .unwrap();
        borrow_limit
    }

    fn liquidation_limit_with_cf(
        user_collateral: u64,
        liquidation_cf_bps: u16,
        collateral_reserve: u64,
        debt_reserve: u64,
        ema_price: u64,
    ) -> u64 {
        let (collateral_ema_reserve, debt_ema_reserve) =
            construct_virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                ema_price,
                ema_price,
            )
            .unwrap();
        let collateral_value_with_impact = CPCurve::calculate_amount_out(
            collateral_ema_reserve,
            debt_ema_reserve,
            user_collateral,
        )
        .unwrap();

        ((collateral_value_with_impact as u128) * (liquidation_cf_bps as u128)
            / (BPS_DENOMINATOR as u128)) as u64
    }

    fn refreshed_liquidation_limit(
        user_collateral: u64,
        total_debt: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        ema_price: u64,
        directional_ema_price: u64,
    ) -> u64 {
        let (_, _, liquidation_cf_bps) = pessimistic_max_debt(
            user_collateral,
            ema_price,
            directional_ema_price,
            collateral_reserve,
            debt_reserve,
            total_debt,
            None,
        )
        .unwrap();

        liquidation_limit_with_cf(
            user_collateral,
            liquidation_cf_bps,
            collateral_reserve,
            debt_reserve,
            ema_price,
        )
    }

    #[test]
    fn max_sentinel_only_resolves_to_all_collateral_without_debt() {
        assert_eq!(simulate_resolve_remove_collateral_amount(123, 0, u64::MAX), 123);
        assert_eq!(simulate_resolve_remove_collateral_amount(123, 1, u64::MAX), u64::MAX);
        assert_eq!(simulate_resolve_remove_collateral_amount(123, 1, 10), 10);
    }

    #[test]
    fn post_withdraw_check_rejects_linear_exploit_withdrawals() {
        let collateral_reserve = 1_000_000;
        let debt_reserve = 1_000_000;
        let ema_price = NAD;
        let directional_ema_price = NAD;
        let cases = [
            (100_000, "10% of reserve"),
            (200_000, "20% of reserve"),
            (300_000, "30% of reserve"),
            (500_000, "50% of reserve"),
            (700_000, "70% of reserve"),
        ];

        for (user_collateral, label) in cases {
            let (user_debt, _, stored_liquidation_cf_bps) = pessimistic_max_debt(
                user_collateral,
                ema_price,
                directional_ema_price,
                collateral_reserve,
                debt_reserve,
                0,
                None,
            )
            .unwrap();
            let initial_liquidation_limit = liquidation_limit_with_cf(
                user_collateral,
                stored_liquidation_cf_bps,
                collateral_reserve,
                debt_reserve,
                ema_price,
            );
            assert!(
                user_debt < initial_liquidation_limit,
                "borrowed position should start non-liquidatable for {}",
                label
            );

            let linear_max_withdrawable = simulate_linear_max_withdrawable(
                user_collateral,
                user_debt,
                user_debt,
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            );
            let linear_remaining = user_collateral - linear_max_withdrawable;
            let linear_refreshed_liquidation_limit = refreshed_liquidation_limit(
                linear_remaining,
                user_debt,
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            );
            assert!(
                linear_max_withdrawable > 0,
                "linear path should reproduce a non-zero vulnerable withdrawal for {}",
                label
            );
            assert!(
                user_debt >= linear_refreshed_liquidation_limit,
                "linear path should leave the position liquidatable for {}",
                label
            );
            assert!(
                post_withdraw_borrow_limit(
                    linear_remaining,
                    user_debt,
                    collateral_reserve,
                    debt_reserve,
                    ema_price,
                    directional_ema_price,
                ) < user_debt,
                "post-withdraw borrow-limit check should reject the linear withdrawal for {}",
                label
            );
        }
    }

    #[test]
    fn post_withdraw_check_rejects_impact_inverse_dynamic_cf_counterexample() {
        let collateral_reserve = 1_000_000;
        let debt_reserve = 1_000_000;
        let ema_price = NAD;
        let directional_ema_price = NAD;
        let user_collateral = 2_000_000;
        let user_debt = 134_583;
        let total_debt = user_debt;

        let impact_inverse_withdrawal = simulate_impact_inverse_max_withdrawable(
            user_collateral,
            user_debt,
            total_debt,
            collateral_reserve,
            debt_reserve,
            ema_price,
            directional_ema_price,
        );
        let remaining = user_collateral - impact_inverse_withdrawal;
        let refreshed_limit = refreshed_liquidation_limit(
            remaining,
            total_debt,
            collateral_reserve,
            debt_reserve,
            ema_price,
            directional_ema_price,
        );

        assert_eq!(remaining, 208_039);
        assert!(
            post_withdraw_borrow_limit(
                remaining,
                total_debt,
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            ) < user_debt,
            "post-withdraw borrow-limit check should reject the one-step impact inverse"
        );
        assert!(
            user_debt >= refreshed_limit,
            "one-step impact inverse should reproduce the dynamic-CF liquidation regression"
        );
    }

    #[test]
    fn post_withdraw_check_accepts_safe_explicit_withdrawal() {
        let collateral_reserve = 1_000_000;
        let debt_reserve = 1_000_000;
        let ema_price = NAD;
        let directional_ema_price = NAD;
        let user_collateral = 2_000_000;
        let user_debt = 134_583;
        let total_debt = user_debt;

        let safe_remaining = 226_185;
        let safe_withdrawal = user_collateral - safe_remaining;
        let refreshed_limit = refreshed_liquidation_limit(
            safe_remaining,
            total_debt,
            collateral_reserve,
            debt_reserve,
            ema_price,
            directional_ema_price,
        );

        assert_eq!(safe_withdrawal, 1_773_815);
        assert!(
            post_withdraw_borrow_limit(
                safe_remaining,
                total_debt,
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            ) >= user_debt,
            "safe explicit withdrawal should keep the debt under the post-withdraw borrow limit"
        );
        assert!(
            user_debt < refreshed_limit,
            "safe explicit withdrawal should keep the position non-liquidatable"
        );
        assert!(
            post_withdraw_borrow_limit(
                safe_remaining - 1,
                total_debt,
                collateral_reserve,
                debt_reserve,
                ema_price,
                directional_ema_price,
            ) < user_debt,
            "regression should pin the minimum safe remaining collateral"
        );

        assert_eq!(user_collateral - (safe_withdrawal + 1), safe_remaining - 1);
    }
}
