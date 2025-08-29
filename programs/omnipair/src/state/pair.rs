use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::gamm_math::{pessimistic_max_debt, pessimistic_min_collateral};
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
    pub config: Pubkey,
    // pair parameters
    pub rate_model: Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub pool_deployer_fee_bps: u16,
    
    // Reserves
    pub reserve0: u64,
    pub reserve1: u64,
    
    // Price tracking
    pub last_price0_ema: u64,
    pub last_price1_ema: u64,
    pub last_update: i64,
    
    // Rates
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
        config: Pubkey,
        rate_model: Pubkey,
        swap_fee_bps: u16,
        half_life: u64,
        pool_deployer_fee_bps: u16,
        current_time: i64,
        bump: u8,
    ) -> Self {
        Self {
            token0,
            token1,
            token0_decimals,
            token1_decimals,
            config,
            // pair parameters
            rate_model,
            swap_fee_bps,
            half_life,
            pool_deployer_fee_bps,

            last_update: current_time,
            bump,

            reserve0: 0,
            reserve1: 0,
            total_supply: MIN_LIQUIDITY,

            last_price0_ema: 0,
            last_price1_ema: 0,
            last_rate0: RateModel::bps_to_nad(INITIAL_RATE_BPS),
            last_rate1: RateModel::bps_to_nad(INITIAL_RATE_BPS),

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

    pub fn get_collateral_token(&self, collateral_token_mint: &Pubkey) -> Pubkey {
       self.get_token_y(collateral_token_mint)
    }

    pub fn get_debt_token(&self, debt_token_mint: &Pubkey) -> Pubkey {
        self.get_token_y(debt_token_mint)
    }

    pub fn get_token_y(&self, token_y: &Pubkey) -> Pubkey {
        match *token_y == self.token0 {
            true => self.token1,
            false => self.token0,
        }
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
                self.half_life
            )
        }
    }

    pub fn get_rates(&self, rate_model: &Account<RateModel>) -> Result<(u64, u64)> {
        let current_time = Clock::get()?.unix_timestamp;
        let time_elapsed = current_time - self.last_update;

        let (util0, util1) = if self.reserve0 > 0 {
            (
                ((self.total_debt0 as u128 * NAD as u128) / self.reserve0 as u128) as u64, 
                ((self.total_debt1 as u128 * NAD as u128) / self.reserve1 as u128) as u64
            )
        } else {
            (0, 0)
        };

        
        Ok((
            rate_model.calculate_rate(self.last_rate0, time_elapsed as u64, util0).0, 
            rate_model.calculate_rate(self.last_rate1, time_elapsed as u64, util1).0
        ))
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
                self.half_life
            )
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.reserve0 > 0 && self.reserve1 > 0 && self.total_supply > 0
    }

    /// Get the maximum debt and pessimistic collateral factor in BPS for a given collateral amount
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `collateral_token`: The token the user is depositing
    /// - `collateral_amount`: The amount of collateral the user is depositing
    /// 
    /// Returns a tuple containing:
    /// - The maximum debt possible for the given collateral amount
    /// - The pessimistic collateral factor in BPS
    pub fn get_max_debt_and_cf_bps_for_collateral(&self, pair: &Pair, collateral_token: &Pubkey, collateral_amount: u64) -> Result<(u64, u16)> {
        let (
            collateral_ema_price,
            collateral_spot_price,
            debt_amm_reserve,
        ) = match collateral_token == &pair.token0 {
            true => (pair.ema_price0_nad(), pair.spot_price0_nad(), pair.reserve1),
            false => (pair.ema_price1_nad(), pair.spot_price1_nad(), pair.reserve0),
        };

        pessimistic_max_debt(
            collateral_amount,
            collateral_ema_price,
            collateral_spot_price,
            debt_amm_reserve,
        ).map_err(|error| error.into())
    }


        /// Get the minimum collateral and pessimistic collateral factor in BPS for a given debt amount
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// - `debt_amount`: The amount of debt the user is borrowing
    /// 
    /// Returns a tuple containing:
    /// - The minimum collateral required to avoid liquidation
    /// - The pessimistic collateral factor in BPS
    pub fn get_min_collateral_and_cf_bps_for_debt(&self, pair: &Pair, debt_token: &Pubkey, debt_amount: u64) -> Result<(u64, u16)> {
        let (
            collateral_ema_price,
            collateral_spot_price,
            debt_amm_reserve,
        ) = match debt_token == &pair.token0 {
            true => (pair.ema_price1_nad(), pair.spot_price1_nad(), pair.reserve0),
            false => (pair.ema_price0_nad(), pair.spot_price0_nad(), pair.reserve1),
        };

        pessimistic_min_collateral(
            debt_amount,
            collateral_ema_price,
            collateral_spot_price,
            debt_amm_reserve,
        ).map_err(|error| error.into())
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
                    self.half_life
                );
                self.last_price1_ema = compute_ema(
                    self.last_price1_ema,
                    self.last_update,
                    if self.reserve1 > 0 { ((self.reserve0 as u128 * NAD as u128) / self.reserve1 as u128) as u64 } else { 0 },
                    self.half_life
                );
                
                // Calculate utilization rates
                let (util0, util1) = if self.reserve0 > 0 {
                    (
                        ((self.total_debt0 as u128 * NAD as u128) / self.reserve0 as u128) as u64, 
                        ((self.total_debt1 as u128 * NAD as u128) / self.reserve1 as u128) as u64
                    )
                } else {
                    (0, 0)
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

