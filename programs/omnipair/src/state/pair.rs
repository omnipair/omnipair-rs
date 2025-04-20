use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::calc::compute_ema;
use crate::state::RateModel;
use crate::events::UpdatePairEvent;

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

    // PDA bump
    pub bump: u8,
}

impl Pair {
    pub fn initialize(
        token0: Pubkey,
        token1: Pubkey,
        rate_model: Pubkey,
        current_time: i64,
        reserve0: u64,
        reserve1: u64,
        total_supply: u64,
        bump: u8,
    ) -> Self {
        Self {
            token0,
            token1,
            reserve0,
            reserve1,
            rate_model,
            last_update: current_time,
            total_supply,
            bump,

            price0_cumulative_last: 0,
            price1_cumulative_last: 0,
            price0_last: 0,
            price1_last: 0,
            last_price0_ema: 0,
            last_price1_ema: 0,
            last_rate0: MIN_RATE,
            last_rate1: MIN_RATE,

            total_debt0: 0,
            total_debt1: 0,
            total_debt0_shares: 0,
            total_debt1_shares: 0,
            total_collateral0: 0,
            total_collateral1: 0,
        }
    }

    pub fn k(&self) -> u128 {
        self.reserve0 as u128 * self.reserve1 as u128
    }

    pub fn spot_price0_mantissa(&self) -> u64 {
        match self.reserve0 {
            0 => 0,
            _ => self.reserve1 * SCALE / self.reserve0,
        }
    }

    pub fn spot_price1_mantissa(&self) -> u64 {
        match self.reserve1 {
            0 => 0,
            _ => self.reserve0 * SCALE / self.reserve1,
        }
    }

    /// EMA prices scaled by 1e9
    pub fn price0_mantissa(&self) -> u64 {
        compute_ema(
            self.last_price0_ema, 
            self.last_update, 
            self.spot_price0_mantissa(), 
            DEFAULT_HALF_LIFE)
    }

    pub fn price1_mantissa(&self) -> u64 {
        compute_ema(
            self.last_price1_ema, 
            self.last_update, 
            self.spot_price1_mantissa(), 
            DEFAULT_HALF_LIFE)
    }

    pub fn update(&mut self, rate_model: &Account<RateModel>) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        
        if current_time > self.last_update {
            // Update oracles
            let time_elapsed = current_time - self.last_update;
            if time_elapsed > 0 {
                // Update price EMAs
                self.last_price0_ema = compute_ema(
                    self.last_price0_ema,
                    self.last_update,
                    if self.reserve0 > 0 { self.reserve1 * SCALE / self.reserve0 } else { 0 },
                    DEFAULT_HALF_LIFE
                );
                self.last_price1_ema = compute_ema(
                    self.last_price1_ema,
                    self.last_update,
                    if self.reserve1 > 0 { self.reserve0 * SCALE / self.reserve1 } else { 0 },
                    DEFAULT_HALF_LIFE
                );
                
                // Update cumulative prices
                self.price0_cumulative_last += (self.price0_last as u128) * (time_elapsed as u128);
                self.price1_cumulative_last += (self.price1_last as u128) * (time_elapsed as u128);
                
                // Calculate utilization rates
                let util0 = if self.reserve0 > 0 {
                    (self.total_debt0 * SCALE) / self.reserve0
                } else {
                    0
                };
                let util1 = if self.reserve1 > 0 {
                    (self.total_debt1 * SCALE) / self.reserve1
                } else {
                    0
                };
                
                // Calculate new rates
                let (new_rate0, integral0) = rate_model.calculate_rate(
                    self.last_rate0, 
                    time_elapsed as u64, 
                    util0
                );
                let (new_rate1, integral1) = rate_model.calculate_rate(
                    self.last_rate1, 
                    time_elapsed as u64, 
                    util1
                );
                
                // Update rates
                self.last_rate0 = new_rate0;
                self.last_rate1 = new_rate1;
                
                // Calculate and apply interest
                let interest0 = (self.total_debt0 as u128 * integral0 as u128) / SCALE as u128;
                let interest1 = (self.total_debt1 as u128 * integral1 as u128) / SCALE as u128;
                
                self.total_debt0 += interest0 as u64;
                self.total_debt1 += interest1 as u64;
                self.reserve0 += interest0 as u64;
                self.reserve1 += interest1 as u64;
            }
            
            self.last_update = current_time;
            
            // Emit event
            emit!(UpdatePairEvent {
                price0_ema: self.last_price0_ema,
                price1_ema: self.last_price1_ema,
                rate0: self.last_rate0,
                rate1: self.last_rate1,
                timestamp: current_time,
            });
        }
        
        Ok(())
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::find_program_address(&[
            GAMM_PAIR_SEED_PREFIX, 
            self.token0.as_ref(), 
            self.token1.as_ref()
        ], &crate::ID).0
    }
}

#[macro_export]
macro_rules! generate_gamm_pair_seeds {
    ($pair:expr) => {{
        &[
            GAMM_PAIR_SEED_PREFIX,
            $pair.token0.as_ref(),
            $pair.token1.as_ref(),
            &[$pair.bump],
        ]
    }};
}

#[macro_export]
macro_rules! generate_gamm_token_vault_seeds {
    ($pair:expr, $token:expr, $bump:expr) => {{
        &[
            GAMM_TOKEN_VAULT_SEED_PREFIX, 
            $pair.key().as_ref(), 
            $token.key().as_ref(), 
            &[$bump]
        ]
    }};
}

