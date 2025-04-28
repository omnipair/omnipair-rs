use crate::constants::*;
use anchor_lang::prelude::{Clock, *};

pub fn compute_ema(last_ema: u64, last_update: i64, input: u64, half_life: u64) -> u64 {
    let current_time = Clock::get().unwrap().unix_timestamp;
    
    let dt = (current_time - last_update) as u64;
    if dt > 0 && half_life > 0 {
        let exp_time = half_life * NAD / NATURAL_LOG_OF_TWO_NAD;
        let x = dt * NAD / exp_time;
        let alpha = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS);
        (input * (NAD - alpha) + last_ema * alpha) / NAD
    } else {
        last_ema
    }
}

pub fn taylor_exp(x: i64, scale: u64, precision: u64) -> u64 {
    // Choose a suitable n for range reduction
    let n = 10u64;
    // Reduce x by n
    let reduced_x = x / (n as i64);
    // Start with 1 (scaled by `scale`)
    let mut term = scale;
    // Initialize sum with 1 (scaled by `scale`)
    let mut sum = scale;

    // Compute Taylor series terms
    for i in 1..=precision {
        // Compute the next term (scaled) with overflow protection
        term = term.checked_mul(reduced_x as u64)
            .and_then(|t| t.checked_div(i * scale))
            .unwrap_or(0);
        // Add the term to the sum with overflow protection
        sum = sum.checked_add(term).unwrap_or(u64::MAX);
    }

    // Start with 1 (scaled by `scale`)
    let mut result = scale;
    // Raise the result to the power of n with overflow protection
    for _ in 0..n {
        result = result.checked_mul(sum)
            .and_then(|r| r.checked_div(scale))
            .unwrap_or(u64::MAX);
    }

    result
}

// babylonian method (https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Babylonian_method)
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

/// Represents two values normalized to NAD (Normalized Accurate Decimal) format (1e9 precision).
pub struct NormalizedTwoValues {
    pub scaled_a: u64,
    pub scaled_b: u64,
}

/// Normalizes two amounts (a and b) to NAD scale (1e9 precision),
/// adjusting based on each token's decimals relative to NAD.
/// 
/// - If the token has fewer decimals than NAD, the value is multiplied up.
/// - If the token has more decimals than NAD, the value is divided down.
/// - If the token already has NAD decimals, no adjustment is made.
/// - If multiplication would overflow, division fallback is automatically applied.
///
/// This function should not return errors: it guarantees safe normalization even for very large inputs.
///
/// # Arguments
/// - `a`: First value (e.g., collateral, debt, swap amount).
/// - `a_decimals`: The number of decimals of the first value's token.
/// - `b`: Second value (e.g., price, swap rate).
/// - `b_decimals`: The number of decimals of the second value.
///
/// # Returns
/// - `NormalizedTwoValues { scaled_a, scaled_b }` — both values normalized to NAD scale.
///
/// # Example
/// ```
/// let a = 1_000_000; // 1.0 token with 6 decimals
/// let b = 2_000_000_000; // 2.0 price already scaled in 9 decimals (NAD)
/// let normalized = normalize_two_values_to_nad(a, 6, b, 9);
/// assert_eq!(normalized.scaled_a, 1_000_000_000); // scaled_a now in NAD scale
/// assert_eq!(normalized.scaled_b, 2_000_000_000); // scaled_b unchanged
/// ```
pub fn normalize_two_values_to_scale(
    a: u64,
    a_decimals: u8,
    b: u64,
    b_decimals: u8,
) -> NormalizedTwoValues {
    fn scale(value: u64, diff: i32) -> Option<u64> {
        match diff.cmp(&0) {
            std::cmp::Ordering::Equal => Some(value),
            std::cmp::Ordering::Greater => {
                let factor = 10u64.checked_pow(diff as u32)?;
                value
                    .checked_mul(factor)
                    .or_else(|| value.checked_div(factor))
            }
            std::cmp::Ordering::Less => {
                let factor = 10u64.checked_pow((-diff) as u32)?;
                value.checked_div(factor)
            }
        }
    }

    let scaled_a = scale(a, NAD_DECIMALS as i32 - a_decimals as i32).unwrap();
    let scaled_b = scale(b, NAD_DECIMALS as i32 - b_decimals as i32).unwrap();

    NormalizedTwoValues { scaled_a, scaled_b }
}

// Overloaded function for known b scale that is NAD
pub fn normalize_two_values_to_nad(
    a: u64,
    a_decimals: u8,
    b: u64,
) -> NormalizedTwoValues {
    normalize_two_values_to_scale(a, a_decimals, b, NAD_DECIMALS)
}

        