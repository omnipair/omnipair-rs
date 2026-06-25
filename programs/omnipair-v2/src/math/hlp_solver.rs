//! Pure math for the hedged-LP within-swap tracking solver (Phase 2).
//!
//! A 2x-leveraged constant-product LP tracks its deposit asset only in the
//! continuous-rebalancing limit. A single discrete swap of price ratio `r`
//! leaves a tracking gap of `E0 * (sqrt(r) - 1)^2`. That gap can be removed by
//! pre-positioning the vault before the swap with a `Δpre = E0 * (sqrt(r) - 1)`
//! leverage adjustment and finishing with the usual post-swap rebalance.
//!
//! In Omnipair the pre-adjustment is a *price-neutral synthetic deepening*, so
//! it changes the realized `r` (endogenous): the production `Δpre` is the fixed
//! point `a = E0 * (sqrt(r(a)) - 1)`, solved with bounded bisection over the
//! real swap simulator. These functions are the numeraire-only building blocks
//! (loss estimate, closed-form guess, root finder); the market-state
//! orchestration is gated behind `HLP_PRE_SOLVE_ENABLED` at the call site.
//!
//! All ratios/amounts are NAD fixed point (`NAD == 1.0`).

use anchor_lang::prelude::*;

use crate::constants::NAD;
use crate::errors::ErrorCode;

/// Integer square root (floor), Newton's method on u128.
pub fn isqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    // Initial guess: 2^(ceil(bits/2)).
    let mut x = 1u128 << ((128 - value.leading_zeros()).div_ceil(2));
    loop {
        let next = (x + value / x) / 2;
        if next >= x {
            return x;
        }
        x = next;
    }
}

/// `sqrt(r)` in NAD, where `r_nad = r * NAD`. Returns `sqrt(r) * NAD`.
pub fn sqrt_ratio_nad(r_nad: u128) -> Result<u128> {
    // sqrt(r) * NAD = sqrt(r_nad * NAD).
    let scaled = r_nad
        .checked_mul(NAD as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(isqrt(scaled))
}

/// Discrete within-swap tracking loss `E0 * (sqrt(r) - 1)^2`, in NAD.
///
/// Returns 0 for `r <= 1` downside moves are handled symmetrically by the
/// caller via the deleverage path; this estimator is used only to decide
/// whether the solve is worth its compute, so the upside form suffices.
pub fn tracking_loss_nad(equity_nad: u128, r_nad: u128) -> Result<u128> {
    if equity_nad == 0 || r_nad <= NAD as u128 {
        return Ok(0);
    }
    let s = sqrt_ratio_nad(r_nad)?;
    let gap = s
        .checked_sub(NAD as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    // equity * gap^2 / NAD^2
    equity_nad
        .checked_mul(gap)
        .and_then(|value| value.checked_div(NAD as u128))
        .and_then(|value| value.checked_mul(gap))
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

/// Closed-form pre-adjustment magnitude `|E0 * (sqrt(r) - 1)|`, in NAD, plus
/// whether it is a lever-up (`r > 1`) or a deleverage (`r < 1`). Used as the
/// initial bisection guess; the true value is solved against the simulator
/// because the synthetic deepening makes `r` endogenous.
pub fn closed_form_pre_adjustment_nad(equity_nad: u128, r_nad: u128) -> Result<(u128, bool)> {
    let s = sqrt_ratio_nad(r_nad)?;
    let nad = NAD as u128;
    if s >= nad {
        let gap = s - nad;
        let amount = equity_nad
            .checked_mul(gap)
            .and_then(|value| value.checked_div(nad))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok((amount, true))
    } else {
        let gap = nad - s;
        let amount = equity_nad
            .checked_mul(gap)
            .and_then(|value| value.checked_div(nad))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok((amount, false))
    }
}

/// Bounded bisection for a monotonically non-decreasing residual `f` over
/// `[lo, hi]`, returning the smallest `x` with `f(x) >= 0` to tolerance, within
/// `max_iters`. `f` returns the signed residual (negative below the root).
/// Used to solve the endogenous-`r` pre-adjustment fixed point against the real
/// swap simulator without unbounded compute.
pub fn bisect<F>(mut lo: u128, mut hi: u128, max_iters: u32, mut f: F) -> Result<u128>
where
    F: FnMut(u128) -> Result<i128>,
{
    require!(hi >= lo, ErrorCode::MarketMathOverflow);
    for _ in 0..max_iters {
        if hi <= lo + 1 {
            break;
        }
        let mid = lo + (hi - lo) / 2;
        if f(mid)? >= 0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nad(x: u128) -> u128 {
        x * NAD as u128
    }

    #[test]
    fn isqrt_matches_floor_sqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(8), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(1_000_000), 1_000);
        let big = (u64::MAX as u128) * (u64::MAX as u128);
        assert_eq!(isqrt(big), u64::MAX as u128);
    }

    #[test]
    fn sqrt_ratio_of_1_44_is_1_2() {
        // r = 1.44 -> sqrt = 1.2.
        let r = nad(144) / 100;
        assert_eq!(sqrt_ratio_nad(r).unwrap(), nad(12) / 10);
    }

    #[test]
    fn tracking_loss_matches_closed_form() {
        // E0 = 100, r = 1.44 -> loss = 100 * (1.2 - 1)^2 = 100 * 0.04 = 4.
        let loss = tracking_loss_nad(nad(100), nad(144) / 100).unwrap();
        assert_eq!(loss, nad(4));
    }

    #[test]
    fn tracking_loss_is_zero_below_unit_ratio() {
        assert_eq!(tracking_loss_nad(nad(100), nad(1)).unwrap(), 0);
        assert_eq!(tracking_loss_nad(nad(100), nad(8) / 10).unwrap(), 0);
    }

    #[test]
    fn closed_form_pre_adjustment_upside() {
        // E0 = 100, r = 1.44 -> Δpre = 100 * (1.2 - 1) = 20, lever up.
        let (amount, lever_up) = closed_form_pre_adjustment_nad(nad(100), nad(144) / 100).unwrap();
        assert_eq!(amount, nad(20));
        assert!(lever_up);
    }

    #[test]
    fn closed_form_pre_adjustment_downside_is_deleverage() {
        // r = 0.64 -> sqrt = 0.8 -> |Δpre| = 100 * 0.2 = 20, deleverage.
        let (amount, lever_up) = closed_form_pre_adjustment_nad(nad(100), nad(64) / 100).unwrap();
        assert_eq!(amount, nad(20));
        assert!(!lever_up);
    }

    #[test]
    fn bisect_finds_threshold_root() {
        // Residual f(x) = x - 1000 (root at 1000); smallest x with f(x) >= 0.
        let root = bisect(0, 1_000_000, 64, |x| Ok(x as i128 - 1_000)).unwrap();
        assert!(root >= 1_000 && root <= 1_001);
    }

    #[test]
    fn bisect_respects_iteration_budget() {
        // With only a few iterations it cannot fully converge on a wide range,
        // but it must stay within [lo, hi] and not panic.
        let root = bisect(0, u64::MAX as u128, 4, |x| Ok(x as i128 - 5)).unwrap();
        assert!(root <= u64::MAX as u128);
    }
}
