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

/// Total outstanding debt denominated in `asset` (margin fixed plus the
/// opposite-direction hLP vault's borrowed leg), valued at `index_nad`.
fn total_borrowed(market: &Market, asset: MarketAsset, index_nad: u128) -> Result<u128> {
    let (margin_fixed, hlp_shares) = match asset {
        // Base-denominated debt: margin base legs + the quote-hLP's base borrow.
        MarketAsset::Base => (
            market.debt.fixed_base_shares,
            market.quote_hlp_vault.debt_shares,
        ),
        // Quote-denominated debt: margin quote legs + the base-hLP's quote borrow.
        MarketAsset::Quote => (
            market.debt.fixed_quote_shares,
            market.base_hlp_vault.debt_shares,
        ),
    };
    let total_shares = margin_fixed
        .checked_add(hlp_shares)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Debt::shares_to_debt(total_shares, index_nad)
}

#[cfg(test)]
mod tests {
    include!("../tests/transitions/interest.rs");
}
