use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::math::compute_ema;
use crate::state::RateModel;
use crate::events::UpdatePairEvent;

#[account]
pub struct Pair {
    // Token addresses
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    
    // Reserves
    pub reserve0: u64,
    pub reserve1: u64,
    
    // Price tracking
    pub last_price0_ema: u64,
    pub last_price1_ema: u64,
    pub last_update: i64,
    
    // Rate model
    pub rate_model: Pubkey,
    pub last_rate0: u64,
    pub last_rate1: u64,
    
    // Debt tracking
    pub total_debt0: u64,
    pub total_debt1: u64,
    pub total_debt0_shares: u64,
    pub total_debt1_shares: u64,
    
    // LP liquidity tracking
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
        token0_decimals: u8,
        token1_decimals: u8,
        rate_model: Pubkey,
        current_time: i64,
        bump: u8,
    ) -> Self {
        Self {
            token0,
            token1,
            token0_decimals,
            token1_decimals,
            rate_model,
            last_update: current_time,
            bump,

            reserve0: 0,
            reserve1: 0,
            total_supply: 0,

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

    pub fn spot_price0_nad(&self) -> u64 {
        match self.reserve0 {
            0 => 0,
            _ => {
                ((self.reserve1 as u128 * NAD as u128) / self.reserve0 as u128) as u64
            }
        }
    }

    pub fn spot_price1_nad(&self) -> u64 {
        match self.reserve1 {
            0 => 0,
            _ => {
                ((self.reserve0 as u128 * NAD as u128) / self.reserve1 as u128) as u64
            }
        }
    }

    /// EMA prices scaled by 1e9
    pub fn ema_price0_nad(&self) -> u64 {
        if self.reserve0 == 0 {
            0
        } else {
            let spot_price = self.spot_price0_nad();
            compute_ema(
                self.last_price0_ema, 
                self.last_update, 
                spot_price, 
                DEFAULT_HALF_LIFE
            )
        }
    }

    pub fn ema_price1_nad(&self) -> u64 {
        if self.reserve1 == 0 {
            0
        } else {
            let spot_price = self.spot_price1_nad();
            compute_ema(
                self.last_price1_ema, 
                self.last_update, 
                spot_price, 
                DEFAULT_HALF_LIFE
            )
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.reserve0 > 0 && self.reserve1 > 0 && self.total_supply > 0
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
                    if self.reserve0 > 0 { ((self.reserve1 as u128 * NAD as u128) / self.reserve0 as u128) as u64 } else { 0 },
                    DEFAULT_HALF_LIFE
                );
                self.last_price1_ema = compute_ema(
                    self.last_price1_ema,
                    self.last_update,
                    if self.reserve1 > 0 { ((self.reserve0 as u128 * NAD as u128) / self.reserve1 as u128) as u64 } else { 0 },
                    DEFAULT_HALF_LIFE
                );
                
                // Calculate utilization rates
                let util0 = if self.reserve0 > 0 {
                    (self.total_debt0 * NAD) / self.reserve0
                } else {
                    0
                };
                let util1 = if self.reserve1 > 0 {
                    (self.total_debt1 * NAD) / self.reserve1
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
                let interest0 = (self.total_debt0 as u128 * integral0 as u128) / NAD as u128;
                let interest1 = (self.total_debt1 as u128 * integral1 as u128) / NAD as u128;
                
                self.total_debt0 += interest0 as u64;
                self.total_debt1 += interest1 as u64;
                // TODO: review this    
                // this applies accrued interest as instant liquidity by appending it to the reserves
                // it applies positive price impact to assets that may be borrowed
                // this can lead to: 
                // 1. virtually pumping up collateral prices without real buying pressure (assuming no arbitrage)
                // 2. lower virtual utilization rates
                // 3. these changes will affect spot & ema prices
                // 4. affecting borrowing power and effective collateral factor
                // 5. affecting liquidation thresholds
                // 6. affecting the amount of debt that can be borrowed
                // 7. affecting the amount of interest that is earned
                self.reserve0 += interest0 as u64;
                self.reserve1 += interest1 as u64;
            }
            
            self.last_update = current_time;
            
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
}

#[macro_export]
macro_rules! generate_gamm_pair_seeds {
    ($pair:expr) => {{
        &[
            PAIR_SEED_PREFIX,
            $pair.token0.as_ref(),
            $pair.token1.as_ref(),
            &[$pair.bump],
        ]
    }};
}

