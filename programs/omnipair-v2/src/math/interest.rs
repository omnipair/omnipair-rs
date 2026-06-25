//! Adaptive-curve borrow interest rate model and borrow-index accrual.
//!
//! The instantaneous borrow APR is a fixed-shape curve anchored at the target
//! utilization and scaled by a per-market `rate_at_target`:
//!
//! ```text
//! rate(u) = rate_at_target * curve(error(u))
//! error(u) = (u - u*)/u*           for u <= u*   (in [-1, 0])
//!          = (u - u*)/(1 - u*)      for u >  u*   (in (0, 1])
//! curve(e) = 1 + (k-1)*e            for e >= 0    (-> k at e=1)
//!          = 1 - (1 - 1/k)*|e|      for e <  0    (-> 1/k at e=-1)
//! ```
//!
//! The curve gives an immediate, graded response to utilization. The anchor
//! `rate_at_target` then drifts over time toward whatever level holds util at
//! target — `rate_at_target *= e^(speed * error * dt/year)` — so the *level* is
//! market-driven rather than hardcoded. Both the index (`shares * index` debt)
//! and the anchor are stored state, so the model is fully reproducible.
//!
//! All rates/ratios are NAD fixed point (`NAD == 1.0`, i.e. 100% APR == NAD).

use anchor_lang::prelude::*;

use crate::constants::{BPS_DENOMINATOR, MAX_INTEREST_ACCRUAL_MS, MS_PER_YEAR, NAD};
use crate::errors::ErrorCode;

/// Utilization of a side, in bps, as `borrowed / (borrowed + idle_cash)`.
/// Returns 0 when nothing is supplied, clamped to `BPS_DENOMINATOR`.
pub fn utilization_bps(borrowed: u128, idle_cash: u128) -> Result<u64> {
    let supplied = borrowed
        .checked_add(idle_cash)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if supplied == 0 {
        return Ok(0);
    }
    let util = borrowed
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(supplied))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(u64::try_from(util.min(BPS_DENOMINATOR as u128)).unwrap_or(BPS_DENOMINATOR as u64))
}

/// Normalized utilization error in NAD, in `[-NAD, NAD]` (0 at target).
pub fn utilization_error_nad(utilization_bps: u64, target_bps: u64) -> Result<i128> {
    let bps = BPS_DENOMINATOR as i128;
    let t = target_bps as i128;
    require!(t > 0 && t < bps, ErrorCode::InvalidMarketConfig);
    let u = (utilization_bps.min(BPS_DENOMINATOR as u64)) as i128;
    let nad = NAD as i128;
    let err = if u <= t {
        (u - t)
            .checked_mul(nad)
            .and_then(|value| value.checked_div(t))
            .ok_or(ErrorCode::MarketMathOverflow)?
    } else {
        (u - t)
            .checked_mul(nad)
            .and_then(|value| value.checked_div(bps - t))
            .ok_or(ErrorCode::MarketMathOverflow)?
    };
    Ok(err)
}

/// Curve multiplier in NAD for a normalized error, ranging `[NAD/steepness, steepness]`
/// and equal to `NAD` at the target (error 0).
pub fn curve_multiplier_nad(error_nad: i128, steepness_nad: u128) -> Result<u128> {
    let nad = NAD as i128;
    let steep = i128::try_from(steepness_nad).map_err(|_| ErrorCode::MarketMathOverflow)?;
    require!(steep >= nad, ErrorCode::InvalidMarketConfig);
    let mult = if error_nad >= 0 {
        // NAD + (steepness - NAD) * error / NAD
        nad.checked_add(
            (steep - nad)
                .checked_mul(error_nad)
                .and_then(|value| value.checked_div(nad))
                .ok_or(ErrorCode::MarketMathOverflow)?,
        )
        .ok_or(ErrorCode::MarketMathOverflow)?
    } else {
        // NAD - (NAD - NAD^2/steepness) * |error| / NAD
        let inv_steep = nad
            .checked_mul(nad)
            .and_then(|value| value.checked_div(steep))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let drop = (nad - inv_steep)
            .checked_mul(-error_nad)
            .and_then(|value| value.checked_div(nad))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        nad.checked_sub(drop).ok_or(ErrorCode::MarketMathOverflow)?
    };
    require!(mult > 0, ErrorCode::MarketMathOverflow);
    Ok(mult as u128)
}

/// Instantaneous borrow APR (NAD) = `rate_at_target * curve(error)`.
pub fn instantaneous_rate_apr_nad(
    rate_at_target_nad: u128,
    error_nad: i128,
    steepness_nad: u128,
) -> Result<u128> {
    let mult = curve_multiplier_nad(error_nad, steepness_nad)?;
    rate_at_target_nad
        .checked_mul(mult)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

/// Drift the anchor: `rate_at_target *= e^(speed * error * dt/year)`, using a
/// bounded linear approximation `(1 + exponent)` of the exponential and clamped
/// to `[min, max]`. Above target (error > 0) the anchor rises; below, it falls.
#[allow(clippy::too_many_arguments)]
pub fn adapt_rate_at_target_nad(
    rate_at_target_nad: u128,
    error_nad: i128,
    dt_ms: u64,
    speed_per_year: u128,
    min_nad: u128,
    max_nad: u128,
    max_step_nad: i128,
) -> Result<u128> {
    if dt_ms == 0 || error_nad == 0 {
        return Ok(rate_at_target_nad.clamp(min_nad, max_nad));
    }
    let dt = dt_ms.min(MAX_INTEREST_ACCRUAL_MS) as i128;
    // exponent (NAD) = speed * error * dt / year
    let exponent = (speed_per_year as i128)
        .checked_mul(error_nad)
        .and_then(|value| value.checked_mul(dt))
        .and_then(|value| value.checked_div(MS_PER_YEAR as i128))
        .ok_or(ErrorCode::MarketMathOverflow)?
        .clamp(-max_step_nad, max_step_nad);
    // factor = max(0, NAD + exponent); linear approximation of e^exponent.
    let factor = (NAD as i128 + exponent).max(0) as u128;
    let next = rate_at_target_nad
        .checked_mul(factor)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(next.clamp(min_nad, max_nad))
}

/// Advance a borrow index by `dt_ms` at the given instantaneous APR (NAD):
/// `index *= 1 + apr * dt / year`. Elapsed time is capped per call.
pub fn accrued_index_nad(index_nad: u128, rate_apr_nad: u128, dt_ms: u64) -> Result<u128> {
    if index_nad == 0 || dt_ms == 0 || rate_apr_nad == 0 {
        return Ok(index_nad);
    }
    let dt = dt_ms.min(MAX_INTEREST_ACCRUAL_MS) as u128;
    let growth_nad = rate_apr_nad
        .checked_mul(dt)
        .and_then(|value| value.checked_div(MS_PER_YEAR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if growth_nad == 0 {
        return Ok(index_nad);
    }
    let delta = index_nad
        .checked_mul(growth_nad)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    index_nad
        .checked_add(delta)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

/// Split a debt repayment into the principal returned to the reserve and the
/// interest owed to suppliers, proportional to the outstanding debt.
///
/// `total_debt = shares * index >= principal`, `repaid <= total_debt`. The
/// reusable primitive for non-compounding interest routing: the interest
/// portion is what a repay/close/liquidate path should move into the interest
/// vault instead of leaving in the reserve. Principal is rounded **down** (so
/// interest rounds up), ensuring the interest vault is never under-funded.
pub fn realized_interest_split(
    repaid: u64,
    total_debt: u128,
    principal: u128,
) -> Result<(u64, u64)> {
    require!(total_debt >= principal, ErrorCode::MarketMathOverflow);
    let repaid_u = repaid as u128;
    require!(repaid_u <= total_debt, ErrorCode::InsufficientDebt);
    if repaid_u == 0 {
        return Ok((0, 0));
    }
    let principal_paid = if repaid_u == total_debt {
        principal
    } else {
        principal
            .checked_mul(repaid_u)
            .and_then(|value| value.checked_div(total_debt))
            .ok_or(ErrorCode::MarketMathOverflow)?
    };
    let interest_paid = repaid_u
        .checked_sub(principal_paid)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let principal_paid = u64::try_from(principal_paid).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let interest_paid = u64::try_from(interest_paid).map_err(|_| ErrorCode::MarketMathOverflow)?;
    Ok((principal_paid, interest_paid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        INTEREST_ADJUSTMENT_SPEED_PER_YEAR, INTEREST_CURVE_STEEPNESS_NAD,
        INTEREST_MAX_ADAPTATION_STEP_NAD, INTEREST_MAX_RATE_AT_TARGET_NAD,
        INTEREST_MIN_RATE_AT_TARGET_NAD, INTEREST_TARGET_UTILIZATION_BPS,
    };

    const TARGET: u64 = INTEREST_TARGET_UTILIZATION_BPS;
    const STEEP: u128 = INTEREST_CURVE_STEEPNESS_NAD;

    fn nad(x: u128) -> u128 {
        x * NAD as u128
    }

    #[test]
    fn utilization_is_ratio_of_borrowed_to_supplied() {
        assert_eq!(utilization_bps(600, 400).unwrap(), 6_000);
        assert_eq!(utilization_bps(1_000, 0).unwrap(), 10_000);
        assert_eq!(utilization_bps(0, 0).unwrap(), 0);
    }

    #[test]
    fn error_is_zero_at_target_and_signed_around_it() {
        assert_eq!(utilization_error_nad(TARGET, TARGET).unwrap(), 0);
        // full utilization -> +NAD
        assert_eq!(utilization_error_nad(10_000, TARGET).unwrap(), NAD as i128);
        // zero utilization -> -NAD
        assert_eq!(utilization_error_nad(0, TARGET).unwrap(), -(NAD as i128));
    }

    #[test]
    fn curve_multiplier_spans_reciprocal_to_steepness() {
        // error 0 -> 1.0
        assert_eq!(curve_multiplier_nad(0, STEEP).unwrap(), NAD as u128);
        // error +1 -> steepness (4x)
        assert_eq!(curve_multiplier_nad(NAD as i128, STEEP).unwrap(), STEEP);
        // error -1 -> 1/steepness (0.25x)
        assert_eq!(
            curve_multiplier_nad(-(NAD as i128), STEEP).unwrap(),
            nad(1) / 4
        );
    }

    #[test]
    fn instantaneous_rate_scales_anchor_by_curve() {
        let anchor = nad(10) / 100; // 10% APR
                                    // at target -> equals anchor
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, 0, STEEP).unwrap(),
            anchor
        );
        // at full util -> 4x anchor
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, NAD as i128, STEEP).unwrap(),
            anchor * 4
        );
        // at zero util -> anchor/4
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, -(NAD as i128), STEEP).unwrap(),
            anchor / 4
        );
    }

    #[test]
    fn anchor_rises_above_target_and_falls_below() {
        let anchor = nad(10) / 100;
        let up = adapt_rate_at_target_nad(
            anchor,
            NAD as i128, // util at 100%
            MS_PER_YEAR / 52,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        let down = adapt_rate_at_target_nad(
            anchor,
            -(NAD as i128),
            MS_PER_YEAR / 52,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert!(up > anchor, "anchor should rise above target");
        assert!(down < anchor, "anchor should fall below target");
    }

    #[test]
    fn anchor_is_clamped_to_bounds() {
        // Already at max, sustained high util -> stays at max.
        let capped = adapt_rate_at_target_nad(
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            NAD as i128,
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(capped, INTEREST_MAX_RATE_AT_TARGET_NAD);
        // Already at min, sustained low util -> stays at min.
        let floored = adapt_rate_at_target_nad(
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            -(NAD as i128),
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(floored, INTEREST_MIN_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn anchor_does_not_move_at_target() {
        let anchor = nad(7) / 100;
        let same = adapt_rate_at_target_nad(
            anchor,
            0,
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(same, anchor);
    }

    #[test]
    fn index_grows_by_apr_over_one_year() {
        // 10% APR for a year -> index * 1.10.
        let index = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR).unwrap();
        assert_eq!(index, nad(110) / 100);
    }

    #[test]
    fn index_unchanged_with_no_time_or_zero_rate() {
        assert_eq!(accrued_index_nad(nad(1), nad(10) / 100, 0).unwrap(), nad(1));
        assert_eq!(accrued_index_nad(nad(1), 0, MS_PER_YEAR).unwrap(), nad(1));
    }

    #[test]
    fn index_elapsed_time_is_capped() {
        let capped = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR * 100).unwrap();
        let one_year = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR).unwrap();
        assert_eq!(capped, one_year);
    }

    #[test]
    fn full_repay_splits_exactly_principal_and_interest() {
        assert_eq!(realized_interest_split(110, 110, 100).unwrap(), (100, 10));
    }

    #[test]
    fn partial_repay_splits_proportionally() {
        assert_eq!(realized_interest_split(55, 110, 100).unwrap(), (50, 5));
    }

    #[test]
    fn no_accrued_interest_routes_nothing() {
        assert_eq!(realized_interest_split(100, 100, 100).unwrap(), (100, 0));
        assert_eq!(realized_interest_split(40, 100, 100).unwrap(), (40, 0));
    }

    #[test]
    fn repay_above_debt_is_rejected() {
        assert_eq!(
            realized_interest_split(120, 110, 100).unwrap_err(),
            error!(ErrorCode::InsufficientDebt)
        );
    }

    #[test]
    fn principal_rounds_down_so_interest_is_never_underfunded() {
        assert_eq!(realized_interest_split(1, 3, 2).unwrap(), (0, 1));
    }
}
