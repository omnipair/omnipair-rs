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
    let n = 10u64; // Choose a suitable n for range reduction
    let reduced_x = x / (n as i64);
    let mut term = scale;
    let mut sum = scale;

    for i in 1..=precision {
        term = (term * (reduced_x as u64)) / (i * scale);
        sum += term;
    }

    let mut result = scale;
    for _ in 0..n {
        result = (result * sum) / scale;
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

        