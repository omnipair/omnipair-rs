use anchor_lang::prelude::*;

#[account]
pub struct RateModel {
    pub exp_rate: u64,
}

impl RateModel {
    pub fn new(exp_rate: u64) -> Self {
        Self { exp_rate }
    }
}
