use crate::constants::*;
use crate::utils::math::taylor_exp;

pub fn calculate_rate(
    last_rate: u64,
    time_elapsed: u64,
    last_util: u64,
    exp_rate: u64,
    target_util_start: u64,
    target_util_end: u64,
) -> (u64, u64) {
    let x = exp_rate * time_elapsed;
    let growth_decay = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS);

    let (curr_borrow_rate, integral) = if last_util > target_util_end {
        let curr_rate = last_rate * NAD / growth_decay;
        let integral = (curr_rate - last_rate) * NAD / exp_rate / SECONDS_PER_YEAR;
        (curr_rate, integral)
    } else if last_util < target_util_start {
        let mut curr_rate = last_rate * growth_decay / NAD;
        if curr_rate < MIN_RATE {
            curr_rate = MIN_RATE;
            let integral = if last_rate <= MIN_RATE {
                // Already at min rate, just use flat rate for entire period
                MIN_RATE * time_elapsed / SECONDS_PER_YEAR
            } else {
                // Calculate time until min rate is reached
                let time_to_min = taylor_exp(-((MIN_RATE * NAD / last_rate) as i64), NAD, TAYLOR_TERMS) * NAD / exp_rate;
                // Decaying integral up to min rate, then add flat rate portion
                ((last_rate - MIN_RATE) * NAD / exp_rate + MIN_RATE * (time_elapsed - time_to_min)) / SECONDS_PER_YEAR
            };
            (curr_rate, integral)
        } else {
            let integral = (last_rate - curr_rate) * NAD / exp_rate / SECONDS_PER_YEAR;
            (curr_rate, integral)
        }
    } else {
        let integral = last_rate * time_elapsed / SECONDS_PER_YEAR;
        (last_rate, integral)
    };

    (curr_borrow_rate, integral)
} 