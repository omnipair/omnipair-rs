//! Borrow interest rate model and borrow-index accrual.
//!
//! The borrow index is a NAD fixed-point accumulator (`NAD == 1.0`). Each
//! accrual multiplies the index by `1 + apr * dt / year`, where `apr` is read
//! off a kinked utilization curve parameterized per market. Because outstanding
//! debt is valued as `shares * index`, advancing the index is exactly what
//! charges interest to borrowers; the matching credit is realized when that
//! debt is repaid back into the reserve (see `transitions::interest`).

use anchor_lang::prelude::*;

use crate::constants::{BPS_DENOMINATOR, MAX_INTEREST_ACCRUAL_MS, MS_PER_YEAR, NAD};
use crate::errors::ErrorCode;

/// Per-market parameters of the kinked borrow-rate curve, all APRs in bps:
///
/// ```text
/// rate(u) = base + slope1 * u / u*                 for u <= u*
///         = base + slope1 + slope2 * (u-u*)/(1-u*) for u >  u*
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InterestRateParams {
    pub base_rate_bps: u64,
    pub slope1_bps: u64,
    pub optimal_utilization_bps: u64,
    pub slope2_bps: u64,
}

impl InterestRateParams {
    /// Borrow APR (in bps) for a given utilization, from the kinked curve.
    pub fn borrow_rate_apr_bps(&self, utilization_bps: u64) -> Result<u64> {
        let bps = BPS_DENOMINATOR as u128;
        let optimal = self.optimal_utilization_bps as u128;
        let util = (utilization_bps as u128).min(bps);

        let apr = if util <= optimal {
            // base + slope1 * util / optimal
            let kink_share = if optimal == 0 {
                0
            } else {
                (self.slope1_bps as u128)
                    .checked_mul(util)
                    .and_then(|value| value.checked_div(optimal))
                    .ok_or(ErrorCode::MarketMathOverflow)?
            };
            (self.base_rate_bps as u128)
                .checked_add(kink_share)
                .ok_or(ErrorCode::MarketMathOverflow)?
        } else {
            // base + slope1 + slope2 * (util - optimal) / (BPS - optimal)
            let above = util
                .checked_sub(optimal)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            let span = bps
                .checked_sub(optimal)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            let steep = if span == 0 {
                self.slope2_bps as u128
            } else {
                (self.slope2_bps as u128)
                    .checked_mul(above)
                    .and_then(|value| value.checked_div(span))
                    .ok_or(ErrorCode::MarketMathOverflow)?
            };
            (self.base_rate_bps as u128)
                .checked_add(self.slope1_bps as u128)
                .and_then(|value| value.checked_add(steep))
                .ok_or(ErrorCode::MarketMathOverflow)?
        };
        u64::try_from(apr).map_err(|_| ErrorCode::MarketMathOverflow.into())
    }
}

/// Utilization of a side, in bps, as `borrowed / (borrowed + idle_cash)`.
/// Returns 0 when nothing is supplied, and is clamped to `BPS_DENOMINATOR`.
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

/// Advance a borrow index forward by `dt_ms` at the APR implied by
/// `utilization_bps` under `params`.
///
/// `index_new = index * (1 + apr * dt / year)` in NAD fixed point. The elapsed
/// time charged in a single call is capped at `MAX_INTEREST_ACCRUAL_MS`.
pub fn accrued_index_nad(
    index_nad: u128,
    params: &InterestRateParams,
    utilization_bps: u64,
    dt_ms: u64,
) -> Result<u128> {
    if index_nad == 0 || dt_ms == 0 {
        return Ok(index_nad);
    }
    let apr_bps = params.borrow_rate_apr_bps(utilization_bps)?;
    if apr_bps == 0 {
        return Ok(index_nad);
    }
    let dt = dt_ms.min(MAX_INTEREST_ACCRUAL_MS) as u128;
    // growth_nad = apr_fraction * dt / year, scaled by NAD.
    let growth_nad = (apr_bps as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_mul(dt))
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        INTEREST_BASE_RATE_BPS, INTEREST_OPTIMAL_UTILIZATION_BPS, INTEREST_SLOPE1_BPS,
        INTEREST_SLOPE2_BPS,
    };

    fn default_params() -> InterestRateParams {
        InterestRateParams {
            base_rate_bps: INTEREST_BASE_RATE_BPS,
            slope1_bps: INTEREST_SLOPE1_BPS,
            optimal_utilization_bps: INTEREST_OPTIMAL_UTILIZATION_BPS,
            slope2_bps: INTEREST_SLOPE2_BPS,
        }
    }

    #[test]
    fn utilization_is_zero_when_nothing_supplied() {
        assert_eq!(utilization_bps(0, 0).unwrap(), 0);
    }

    #[test]
    fn utilization_is_ratio_of_borrowed_to_supplied() {
        // 600 borrowed, 400 idle -> 60%.
        assert_eq!(utilization_bps(600, 400).unwrap(), 6_000);
        // fully borrowed -> clamped at 100%.
        assert_eq!(utilization_bps(1_000, 0).unwrap(), 10_000);
    }

    #[test]
    fn rate_curve_is_kinked_at_optimal() {
        let params = default_params();
        // base = 0, slope1 = 1000 over optimal = 8000.
        assert_eq!(params.borrow_rate_apr_bps(0).unwrap(), INTEREST_BASE_RATE_BPS);
        // halfway to the kink -> half of slope1.
        assert_eq!(params.borrow_rate_apr_bps(4_000).unwrap(), 500);
        // at the kink -> base + slope1.
        assert_eq!(
            params
                .borrow_rate_apr_bps(INTEREST_OPTIMAL_UTILIZATION_BPS)
                .unwrap(),
            INTEREST_BASE_RATE_BPS + INTEREST_SLOPE1_BPS
        );
        // full utilization -> base + slope1 + slope2.
        assert_eq!(
            params.borrow_rate_apr_bps(10_000).unwrap(),
            INTEREST_BASE_RATE_BPS + INTEREST_SLOPE1_BPS + INTEREST_SLOPE2_BPS
        );
    }

    #[test]
    fn rate_curve_is_monotonic_non_decreasing() {
        let params = default_params();
        let mut last = 0;
        for util in (0..=10_000).step_by(250) {
            let rate = params.borrow_rate_apr_bps(util).unwrap();
            assert!(rate >= last, "rate dropped at util {}", util);
            last = rate;
        }
    }

    #[test]
    fn custom_params_change_the_curve() {
        // A flat 5% APR curve with the kink at 50%.
        let flat = InterestRateParams {
            base_rate_bps: 500,
            slope1_bps: 0,
            optimal_utilization_bps: 5_000,
            slope2_bps: 0,
        };
        assert_eq!(flat.borrow_rate_apr_bps(0).unwrap(), 500);
        assert_eq!(flat.borrow_rate_apr_bps(10_000).unwrap(), 500);
    }

    #[test]
    fn index_is_unchanged_with_no_elapsed_time() {
        assert_eq!(
            accrued_index_nad(NAD as u128, &default_params(), 10_000, 0).unwrap(),
            NAD as u128
        );
    }

    #[test]
    fn index_is_unchanged_at_zero_rate() {
        // utilization 0 -> base rate 0 -> no growth.
        assert_eq!(
            accrued_index_nad(NAD as u128, &default_params(), 0, MS_PER_YEAR).unwrap(),
            NAD as u128
        );
    }

    #[test]
    fn index_grows_by_apr_over_one_year_at_kink() {
        // At the kink APR = 10%, one year -> index * 1.10.
        let index = accrued_index_nad(
            NAD as u128,
            &default_params(),
            INTEREST_OPTIMAL_UTILIZATION_BPS,
            MS_PER_YEAR,
        )
        .unwrap();
        let expected = (NAD as u128) * 110 / 100;
        assert_eq!(index, expected);
    }

    #[test]
    fn index_growth_is_proportional_to_time() {
        let params = default_params();
        let half = accrued_index_nad(NAD as u128, &params, 10_000, MS_PER_YEAR / 2).unwrap();
        let full = accrued_index_nad(NAD as u128, &params, 10_000, MS_PER_YEAR).unwrap();
        let half_delta = half - NAD as u128;
        let full_delta = full - NAD as u128;
        // Within rounding, full-year growth is twice the half-year growth.
        assert!(full_delta.abs_diff(half_delta * 2) <= 1);
    }

    #[test]
    fn elapsed_time_is_capped_per_accrual() {
        let params = default_params();
        let capped = accrued_index_nad(NAD as u128, &params, 10_000, MS_PER_YEAR * 100).unwrap();
        let one_year = accrued_index_nad(NAD as u128, &params, 10_000, MS_PER_YEAR).unwrap();
        assert_eq!(capped, one_year);
    }
}
