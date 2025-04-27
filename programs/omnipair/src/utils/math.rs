use crate::constants::*;
use anchor_lang::prelude::{Clock, *};

pub fn compute_ema(last_ema: u64, last_update: i64, input: u64, half_life: u64) -> u64 {
    let current_time = Clock::get().unwrap().unix_timestamp;
    
    let dt = (current_time - last_update) as u64;
    if dt > 0 && half_life > 0 {
        let exp_time = half_life * SCALE / SCALED_NATURAL_LOG_OF_TWO;
        let x = dt * SCALE / exp_time;
        let alpha = taylor_exp(-(x as i64), SCALE, TAYLOR_TERMS);
        (input * (SCALE - alpha) + last_ema * alpha) / SCALE
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

        