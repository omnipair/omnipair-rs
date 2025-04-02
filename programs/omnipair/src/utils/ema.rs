use anchor_lang::prelude::*;
use crate::constants::{SCALE, TAYLOR_TERMS, SCALED_NATURAL_LOG_OF_TWO};
use crate::utils::taylor_exp::exp_fn_scaled;

/// Computes the Exponential Moving Average (EMA).
///
/// # Parameters
/// - `last_ema`: The previous EMA value (scaled by SCALE).
/// - `last_update`: The Unix timestamp (in seconds) when the EMA was last updated.
/// - `input`: The new price input (scaled by SCALE).
/// - `half_life`: The half-life in seconds.
///
/// # Returns
/// The new EMA value (scaled by SCALE).
///
/// # Explanation
/// The computation uses the formula:
///   EMA_new = (input * (SCALE - alpha) + last_ema * alpha) / SCALE
/// where alpha = wad_exp(-x) and x = dt * SCALE / exp_time,
/// and exp_time = half_life * SCALE / 693147180559945300
/// (since ln(2) ≈ 0.693147180559945300).
pub fn compute_ema(last_ema: u64, last_update: u64, input: u64, half_life: u64) -> u64 {
    // Get the current time from Solana's Clock.
    let clock = Clock::get().unwrap();
    let current_time = clock.unix_timestamp as u64;
    let dt = current_time.saturating_sub(last_update); // time difference in seconds

    if dt > 0 && half_life > 0 {
        // at 10 minutes half life, this is = 865
        // Compute the effective time constant: half_life / ln(2), scaled.
        let exp_time = half_life.saturating_mul(SCALE) / SCALED_NATURAL_LOG_OF_TWO;
        // x is the time delta scaled relative to the time constant.
        let x = dt.saturating_mul(SCALE) / exp_time;
        // Compute the smoothing factor alpha using a Taylor-series-based exponentiation.
        // The exponent for e^x is negative because EMA uses exponential decay.
        // e^(-x) is approximated using the Taylor series to compute the decay factor.
        let alpha = exp_fn_scaled(-(x as i64), SCALE, TAYLOR_TERMS) as u64;
        // Return the new EMA based on the weighting.
        (input.saturating_mul(SCALE.saturating_sub(alpha)) + last_ema.saturating_mul(alpha)) / SCALE
    } else {
        // If no time has passed or half_life is zero, return the previous EMA.
        last_ema
    }
}
