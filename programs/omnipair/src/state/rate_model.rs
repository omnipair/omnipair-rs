use anchor_lang::prelude::*;
use crate::constants::*;

#[account]
pub struct RateModel {
    pub exp_rate: u64,
    pub target_util_start: u64,
    pub target_util_end: u64,
}

impl RateModel {
    pub fn new() -> Self {
        Self {
            exp_rate: SCALED_NATURAL_LOG_OF_TWO / SECONDS_PER_DAY,
            target_util_start: TARGET_UTIL_START,
            target_util_end: TARGET_UTIL_END,
        }
    }
}
