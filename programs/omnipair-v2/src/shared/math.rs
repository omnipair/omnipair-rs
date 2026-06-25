use anchor_lang::prelude::{Clock, *};

use crate::constants::{NAD, NATURAL_LOG_OF_TWO_NAD, TARGET_MS_PER_SLOT, TAYLOR_TERMS};

/// Approximates the elapsed time in milliseconds between two slots.
pub fn slots_to_ms(start_slot: u64, end_slot: u64) -> Option<u64> {
    end_slot
        .checked_sub(start_slot)?
        .checked_mul(TARGET_MS_PER_SLOT)
}

pub fn compute_ema(last_ema: u64, last_update: u64, input: u64, half_life: u64) -> u64 {
    let current_slot = Clock::get().map(|clock| clock.slot).unwrap_or(last_update);
    let dt = slots_to_ms(last_update, current_slot).unwrap_or(0);

    if dt > 0 && half_life > 0 {
        // Calculate x in NAD scale
        let x = (dt as u128 * NATURAL_LOG_OF_TWO_NAD as u128) / half_life as u128;
        let alpha = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS);

        let result = ((input as u128 * (NAD - alpha) as u128 + last_ema as u128 * alpha as u128)
            / NAD as u128) as u64;
        result
    } else {
        last_ema
    }
}

pub fn taylor_exp(x: i64, scale: u64, precision: u64) -> u64 {
    // For negative x, we calculate exp(-x) and take reciprocal
    let is_negative = x < 0;
    let abs_x = if is_negative { -x } else { x };

    // Choose a suitable n for range reduction
    let n = 10u64;
    // Reduce x by n
    let reduced_x = abs_x / (n as i64);

    // Start with 1 (scaled by `scale`)
    let mut term = scale as u128;
    // Initialize sum with 1 (scaled by `scale`)
    let mut sum = scale as u128;

    // Compute Taylor series terms
    for i in 1..=precision {
        // Compute the next term (scaled) with overflow protection
        term = term
            .checked_mul(reduced_x as u128)
            .and_then(|t| t.checked_div(i as u128 * scale as u128))
            .unwrap_or(0);
        // Add the term to the sum with overflow protection
        sum = sum.checked_add(term).unwrap_or(u128::MAX);
    }

    // Start with 1 (scaled by `scale`)
    let mut result = scale as u128;
    // Raise the result to the power of n with overflow protection
    for _i in 0..n {
        result = result
            .checked_mul(sum)
            .and_then(|r| r.checked_div(scale as u128))
            .unwrap_or(u128::MAX);
    }

    // If x was negative, take reciprocal
    if is_negative {
        result = (scale as u128 * scale as u128) / result;
    }

    result as u64
}

// Babylonian (Newton's) method (https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Babylonian_method)
// Safe sqrt function that returns None if the input is negative
pub trait SqrtU128 {
    fn sqrt(&self) -> Option<u128>;
}

impl SqrtU128 for u128 {
    fn sqrt(&self) -> Option<u128> {
        let y = *self;
        if y > 3 {
            let mut z = y;
            let mut x = y.checked_div(2)?.checked_add(1)?;
            while x < z {
                z = x;
                x = (y.checked_div(x)?.checked_add(x)?).checked_div(2)?;
            }
            Some(z)
        } else if y != 0 {
            Some(1)
        } else {
            Some(0)
        }
    }
}

/// Ceiling division: rounds up to the nearest integer
/// Formula: ceil(a / b) = (a + b - 1) / b
/// Returns None on overflow
pub fn ceil_div(a: u128, b: u128) -> Option<u128> {
    if b == 0 {
        return None;
    }
    a.checked_add(b - 1)?.checked_div(b)
}
