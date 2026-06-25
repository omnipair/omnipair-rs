//! Borrow-index accrual transition (adaptive-curve model).
//!
//! For each side: measure utilization over *all* borrowing against it (margin
//! debt plus the opposite-direction hLP vault's leg, both denominated in that
//! side's asset), price the instantaneous borrow rate off the curve anchored at
//! the side's `rate_at_target`, accrue the borrow index over the elapsed time,
//! and then drift `rate_at_target` toward the target utilization.
//!
//! Advancing the index charges interest (debt is `shares * index`); the anchor
//! drift makes the rate *level* market-driven. Both are stored state, so the
//! result is fully reproducible.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        INTEREST_ADJUSTMENT_SPEED_PER_YEAR, INTEREST_CURVE_STEEPNESS_NAD,
        INTEREST_MAX_ADAPTATION_STEP_NAD, INTEREST_MAX_RATE_AT_TARGET_NAD,
        INTEREST_MIN_RATE_AT_TARGET_NAD, INTEREST_TARGET_UTILIZATION_BPS, TARGET_MS_PER_SLOT,
    },
    errors::ErrorCode,
    math::{
        accrued_index_nad, adapt_rate_at_target_nad, instantaneous_rate_apr_nad, utilization_bps,
        utilization_error_nad,
    },
    state::{Debt, Market, MarketAsset},
};

pub struct AccrueInterest {
    pub current_slot: u64,
}

impl AccrueInterest {
    pub fn new(current_slot: u64) -> Self {
        Self { current_slot }
    }

    pub fn apply(self, market: &mut Market) -> Result<()> {
        let last = market.debt.last_accrual_slot;
        // No forward time elapsed (or the clock moved backwards): nothing to do.
        if self.current_slot <= last {
            return Ok(());
        }
        let dt_ms = self
            .current_slot
            .checked_sub(last)
            .ok_or(ErrorCode::MarketMathOverflow)?
            .saturating_mul(TARGET_MS_PER_SLOT);

        accrue_side(market, MarketAsset::Base, dt_ms)?;
        accrue_side(market, MarketAsset::Quote, dt_ms)?;
        market.debt.last_accrual_slot = self.current_slot;
        Ok(())
    }
}

fn accrue_side(market: &mut Market, asset: MarketAsset, dt_ms: u64) -> Result<()> {
    let (index, rate_at_target) = match asset {
        MarketAsset::Base => (
            market.debt.base_borrow_index_nad,
            market.debt.base_rate_at_target_nad,
        ),
        MarketAsset::Quote => (
            market.debt.quote_borrow_index_nad,
            market.debt.quote_rate_at_target_nad,
        ),
    };
    let cash = match asset {
        MarketAsset::Base => market.base_side.reserves.cash_reserve,
        MarketAsset::Quote => market.quote_side.reserves.cash_reserve,
    } as u128;

    let borrowed = total_borrowed(market, asset, index)?;
    let util = utilization_bps(borrowed, cash)?;
    let error = utilization_error_nad(util, INTEREST_TARGET_UTILIZATION_BPS)?;

    // Accrue the index at the rate that prevailed over the elapsed window
    // (using the anchor as it stood during that window), then drift the anchor.
    let rate = instantaneous_rate_apr_nad(rate_at_target, error, INTEREST_CURVE_STEEPNESS_NAD)?;
    let next_index = accrued_index_nad(index, rate, dt_ms)?;
    let next_rate_at_target = adapt_rate_at_target_nad(
        rate_at_target,
        error,
        dt_ms,
        INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
        INTEREST_MIN_RATE_AT_TARGET_NAD,
        INTEREST_MAX_RATE_AT_TARGET_NAD,
        INTEREST_MAX_ADAPTATION_STEP_NAD,
    )?;

    match asset {
        MarketAsset::Base => {
            market.debt.base_borrow_index_nad = next_index;
            market.debt.base_rate_at_target_nad = next_rate_at_target;
        }
        MarketAsset::Quote => {
            market.debt.quote_borrow_index_nad = next_index;
            market.debt.quote_rate_at_target_nad = next_rate_at_target;
        }
    }
    Ok(())
}

/// Total outstanding debt denominated in `asset` (margin fixed + soft debt plus
/// the opposite-direction hLP vault's borrowed leg), valued at `index_nad`.
fn total_borrowed(market: &Market, asset: MarketAsset, index_nad: u128) -> Result<u128> {
    let (margin_fixed, margin_soft, hlp_shares) = match asset {
        // Base-denominated debt: margin base legs + the quote-hLP's base borrow.
        MarketAsset::Base => (
            market.debt.fixed_base_shares,
            market.debt.soft_base_shares,
            market.quote_hlp_vault.debt_shares,
        ),
        // Quote-denominated debt: margin quote legs + the base-hLP's quote borrow.
        MarketAsset::Quote => (
            market.debt.fixed_quote_shares,
            market.debt.soft_quote_shares,
            market.base_hlp_vault.debt_shares,
        ),
    };
    let total_shares = margin_fixed
        .checked_add(margin_soft)
        .and_then(|value| value.checked_add(hlp_shares))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Debt::shares_to_debt(total_shares, index_nad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{
            INTEREST_INITIAL_RATE_AT_TARGET_NAD, INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MIN_RATE_AT_TARGET_NAD, MS_PER_YEAR, NAD, TARGET_MS_PER_SLOT,
        },
        state::{
            Debt, HlpVault, Insurance, MarketConfig, MarketHealth, MarketSide, Reserves, Risk,
        },
    };

    fn slots_for_ms(ms: u64) -> u64 {
        ms / TARGET_MS_PER_SLOT
    }

    fn test_market(base_cash: u64, quote_cash: u64) -> Market {
        let mut base_side = MarketSide::default();
        base_side.reserves = Reserves {
            live_reserve: base_cash,
            cash_reserve: base_cash,
            reserved_liability: 0,
        };
        let mut quote_side = MarketSide::default();
        quote_side.reserves = Reserves {
            live_reserve: quote_cash,
            cash_reserve: quote_cash,
            reserved_liability: 0,
        };
        Market {
            version: 2,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            manager: Pubkey::new_unique(),
            base_side,
            quote_side,
            config: MarketConfig::default(),
            debt: Debt {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                base_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                quote_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                last_accrual_slot: 0,
                ..Debt::default()
            },
            base_hlp_vault: HlpVault::default(),
            quote_hlp_vault: HlpVault::default(),
            risk: Risk::default(),
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            params_hash: [0u8; 32],
            last_update_slot: 0,
            reduce_only: false,
            bump: 255,
        }
    }

    #[test]
    fn no_time_elapsed_is_a_noop() {
        let mut market = test_market(1_000, 1_000);
        market.debt.last_accrual_slot = 100;
        AccrueInterest::new(100).apply(&mut market).unwrap();
        assert_eq!(market.debt.quote_borrow_index_nad, NAD as u128);
        assert_eq!(
            market.debt.quote_rate_at_target_nad,
            INTEREST_INITIAL_RATE_AT_TARGET_NAD
        );
        assert_eq!(market.debt.last_accrual_slot, 100);
    }

    #[test]
    fn idle_side_drifts_anchor_down_toward_min() {
        // Cash present, zero debt -> utilization 0 -> error -1 -> anchor falls.
        let mut market = test_market(1_000_000, 1_000_000);
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        assert!(market.debt.quote_rate_at_target_nad < INTEREST_INITIAL_RATE_AT_TARGET_NAD);
        assert!(market.debt.quote_rate_at_target_nad >= INTEREST_MIN_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn high_utilization_raises_anchor_and_accrues_index() {
        // Quote borrowed 950 via base-hLP, 50 cash -> util 95% (above 90% target).
        // error = +0.5 -> curve mult 2.5x -> rate = 4% * 2.5 = 10% APR.
        let mut market = test_market(1_000_000, 50);
        market.base_hlp_vault.debt_shares = 950;
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        // 10% APR over one year compounds the index to 1.10.
        assert_eq!(market.debt.quote_borrow_index_nad, (NAD as u128) * 110 / 100);
        // Anchor drifted up (util above target).
        assert!(market.debt.quote_rate_at_target_nad > INTEREST_INITIAL_RATE_AT_TARGET_NAD);
        assert!(market.debt.quote_rate_at_target_nad <= INTEREST_MAX_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn margin_and_hlp_debt_both_count_toward_utilization() {
        // Quote debt = 480 margin + 480 base-hLP = 960 borrowed, 40 cash -> 96%
        // (> target), so the anchor must rise. If either leg were ignored, util
        // would fall below target and the anchor would instead drop.
        let mut market = test_market(1_000_000, 40);
        market.debt.fixed_quote_shares = 480;
        market.base_hlp_vault.debt_shares = 480;
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        assert!(market.debt.quote_rate_at_target_nad > INTEREST_INITIAL_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn anchor_saturates_at_max_under_sustained_pressure() {
        // ~100% utilization held for years: the anchor ramps up (capped per
        // step) and clamps at the max, never exceeding it.
        let mut market = test_market(1_000_000, 1);
        market.base_hlp_vault.debt_shares = 10_000;
        for year in 1..=15u64 {
            AccrueInterest::new(slots_for_ms(MS_PER_YEAR * year))
                .apply(&mut market)
                .unwrap();
        }
        assert_eq!(
            market.debt.quote_rate_at_target_nad,
            INTEREST_MAX_RATE_AT_TARGET_NAD
        );
    }
}
