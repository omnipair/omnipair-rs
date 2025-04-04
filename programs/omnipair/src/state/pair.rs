use anchor_lang::prelude::*;
use crate::constants::*;

#[account]
pub struct Pair {
    // Token addresses
    pub token0: Pubkey,
    pub token1: Pubkey,
    
    // Reserves
    pub reserve0: u64,
    pub reserve1: u64,
    
    // Price tracking
    pub last_update: i64,
    pub price0_cumulative_last: u128,
    pub price1_cumulative_last: u128,
    pub price0_last: u64,
    pub price1_last: u64,
    pub last_price0_ema: u64,
    pub last_price1_ema: u64,
    
    // Rate model
    pub rate_model: Pubkey,
    pub last_rate0: u64,
    pub last_rate1: u64,
    
    // Debt tracking
    pub total_debt0: u64,
    pub total_debt1: u64,
    pub total_debt0_shares: u64,
    pub total_debt1_shares: u64,
    
    // Liquidity tracking
    pub total_supply: u64,
    
    // Collateral tracking
    pub total_collateral0: u64,
    pub total_collateral1: u64,
    
    // Liquidation bond
    pub liquidation_bond: u64,
}

#[account]
pub struct UserState {
    // Collateral
    pub collateral0: u64,
    pub collateral1: u64,
    
    // Debt shares
    pub debt0_shares: u64,
    pub debt1_shares: u64,
    
    // Liquidation bond
    pub liquidation_bond: u64,
    
    // Delegation
    pub delegate: Pubkey,
}

impl Pair {
    pub const SIZE: usize = 
        32 + 32 + // token0, token1
        8 + 8 + // reserve0, reserve1
        8 + 8 + 8 + // last_update, price0_cumulative_last, price1_cumulative_last, price0_last, price1_last
        8 + 8 + 8 + 8 + // last_price0_ema, last_price1_ema
        32 + 8 + 8 + // rate_model, last_rate0, last_rate1
        8 + 8 + 8 + 8 + // total_debt0, total_debt1, total_debt0_shares, total_debt1_shares
        8 + // total_supply
        8 + 8 + // total_collateral0, total_collateral1
        8; // liquidation_bond

    pub fn new(
        token0: Pubkey,
        token1: Pubkey,
        rate_model: Pubkey,
        current_time: i64,
    ) -> Self {
        Self {
            token0,
            token1,
            reserve0: 0,
            reserve1: 0,
            last_update: current_time,
            price0_cumulative_last: 0,
            price1_cumulative_last: 0,
            price0_last: 0,
            price1_last: 0,
            last_price0_ema: 0,
            last_price1_ema: 0,
            rate_model,
            last_rate0: MIN_RATE,
            last_rate1: MIN_RATE,
            total_debt0: 0,
            total_debt1: 0,
            total_debt0_shares: 0,
            total_debt1_shares: 0,
            total_supply: 0,
            total_collateral0: 0,
            total_collateral1: 0,
            liquidation_bond: 0,
        }
    }
}

impl UserState {
    pub const SIZE: usize = 
        8 + 8 + // collateral0, collateral1
        8 + 8 + // debt0_shares, debt1_shares
        8 + // liquidation_bond
        32; // delegate

    pub fn new() -> Self {
        Self {
            collateral0: 0,
            collateral1: 0,
            debt0_shares: 0,
            debt1_shares: 0,
            liquidation_bond: 0,
            delegate: Pubkey::default(),
        }
    }
}
