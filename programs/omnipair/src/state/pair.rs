use anchor_lang::prelude::*;

#[account]
pub struct Pair {
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub reserve0: u64,
    pub reserve1: u64,
    pub last_update: i64,
    pub last_price0_ema: u64,
    pub last_price1_ema: u64,
    pub rate_model: Pubkey,
    pub last_rate0: u64,
    pub last_rate1: u64,
}

impl Pair {
    pub const SIZE: usize = 32 + 32 + 8 + 8 + 8 + 8 + 8 + 32 + 8 + 8;
}
