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
    include!("../tests/math/hlp_solver.rs");
}
