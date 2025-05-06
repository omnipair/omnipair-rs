use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::math::*;

#[account]
pub struct RateModel {
    pub exp_rate: u64,
    pub target_util_start: u64,
    pub target_util_end: u64,
}

impl RateModel {
    pub fn new() -> Self {
        Self {
            exp_rate: NATURAL_LOG_OF_TWO_NAD / SECONDS_PER_DAY,
            target_util_start: TARGET_UTIL_START_NAD,
            target_util_end: TARGET_UTIL_END_NAD,
        }
    }

    pub fn calculate_rate(&self, last_rate: u64, time_elapsed: u64, last_util: u64) -> (u64, u64) {
        let x = self.exp_rate * time_elapsed;
        let growth_decay = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS);
        
        if last_util > self.target_util_end {
            let curr_rate = (last_rate * NAD) / growth_decay;
            let integral = ((curr_rate - last_rate) * NAD) / self.exp_rate / SECONDS_PER_YEAR;
            (curr_rate, integral)
        } else if last_util < self.target_util_start {
            let curr_rate = (last_rate * growth_decay) / NAD;
            if curr_rate < MIN_RATE {
                if last_rate <= MIN_RATE {
                    let integral = (MIN_RATE * time_elapsed) / SECONDS_PER_YEAR;
                    (MIN_RATE, integral)
                } else {
                    let time_to_min = ((NATURAL_LOG_OF_TWO_NAD * NAD) / self.exp_rate) * 
                        ((MIN_RATE * NAD) / last_rate) as u64;
                    let integral = ((last_rate - MIN_RATE) * NAD / self.exp_rate + 
                        MIN_RATE * (time_elapsed - time_to_min)) / SECONDS_PER_YEAR;
                    (MIN_RATE, integral)
                }
            } else {
                let integral = ((last_rate - curr_rate) * NAD) / self.exp_rate / SECONDS_PER_YEAR;
                (curr_rate, integral)
            }
        } else {
            let integral = (last_rate * time_elapsed) / SECONDS_PER_YEAR;
            (last_rate, integral)
        }
    }
}
