use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::gamm_math::pessimistic_max_debt;
use crate::utils::math::compute_ema;
use crate::state::RateModel;
use crate::events::{UpdatePairEvent, EventMetadata};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct VaultBumps {
    pub reserve0: u8,
    pub reserve1: u8,
    pub collateral0: u8,
    pub collateral1: u8,
}

#[account]
pub struct Pair {
    // Token addresses
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub lp_mint: Pubkey,

    // pair parameters
    pub rate_model: Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    // Fixed collateral factor (BPS). If Some, use this instead of dynamic CF
    pub fixed_cf_bps: Option<u16>,
    
    // Reserves
    pub reserve0: u64,
    pub reserve1: u64,

    // Protocol revenue reserves
    pub protocol_revenue_reserve0: u64,
    pub protocol_revenue_reserve1: u64,
    
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

    pub token0_decimals: u8,
    pub token1_decimals: u8,
    
    pub params_hash: [u8; 32],
    pub version: u8,
    pub bump: u8,
    pub vault_bumps: VaultBumps,
}

impl Pair {
    pub fn initialize(
        token0: Pubkey,
        token1: Pubkey,
        lp_mint: Pubkey,
        token0_decimals: u8,
        token1_decimals: u8,
        rate_model: Pubkey,
        swap_fee_bps: u16,
        half_life: u64,
        fixed_cf_bps: Option<u16>,
        current_time: i64,
        params_hash: [u8; 32],
        version: u8,
        bump: u8,
        vault_bumps: VaultBumps,
    ) -> Self {
        Self {
            token0,
            token1,
            lp_mint,
            token0_decimals,
            token1_decimals,

            // pair parameters
            rate_model,
            swap_fee_bps,
            half_life,
            fixed_cf_bps,
            last_update: current_time,

            reserve0: 0,
            reserve1: 0,
            protocol_revenue_reserve0: 0,
            protocol_revenue_reserve1: 0,
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
            params_hash,
            version,
            bump,
            // don't use default values for vault bumps
            vault_bumps,
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
    /// - The maximum collateral factor in BPS
    /// - The liquidation collateral factor in BPS (max_allowed_cf_bps - LTV_BUFFER_BPS)
    /// 
    /// If `fixed_cf_bps` is `Some`, uses the fixed collateral factor instead of dynamic calculation.
    pub fn get_max_debt_and_cf_bps_for_collateral(&self, pair: &Pair, collateral_token: &Pubkey, collateral_amount: u64) -> Result<(u64, u16, u16)> {
        let (
            collateral_ema_price,
            collateral_spot_price,
            collateral_amm_reserve,
            debt_amm_reserve,
            debt_total,
            
        ) = match collateral_token == &pair.token0 {
            true => (pair.ema_price0_nad(), pair.spot_price0_nad(), pair.total_collateral0, pair.reserve1, pair.total_debt1),
            false => (pair.ema_price1_nad(), pair.spot_price1_nad(), pair.total_collateral1, pair.reserve0, pair.total_debt0),
        };

        pessimistic_max_debt(
            collateral_amount,
            collateral_ema_price,
            collateral_spot_price,
            collateral_amm_reserve,
            debt_amm_reserve,
            debt_total,
            pair.fixed_cf_bps,
        )
    }

    pub fn get_reserve_vault_bump(&self, reserve_token_mint: &Pubkey) -> u8 {
        match reserve_token_mint == &self.token0 {
            true => self.vault_bumps.reserve0,
            false => self.vault_bumps.reserve1,
        }
    }

    pub fn get_collateral_vault_bump(&self, collateral_token_mint: &Pubkey) -> u8 {
        match collateral_token_mint == &self.token0 {
            true => self.vault_bumps.collateral0,
            false => self.vault_bumps.collateral1,
        }
    }

    pub fn update(&mut self, rate_model: &Account<RateModel>, futarchy_authority: &crate::state::FutarchyAuthority, pair_key: Pubkey) -> Result<()> {
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
                let total_interest0 = (self.total_debt0 as u128 * integral0 as u128) / NAD as u128;
                let total_interest1 = (self.total_debt1 as u128 * integral1 as u128) / NAD as u128;

                // Calculate protocol fee as an extra fee on top of interest (not a share of interest)
                // Borrowers pay: interest + protocol_fee
                // LPs receive: interest (full amount)
                // Protocol receives: protocol_fee (extra fee charged to borrowers)
                let protocol_fee0: u64 = ((total_interest0 as u128 * futarchy_authority.revenue_share.interest_bps as u128) / BPS_DENOMINATOR as u128) as u64;
                let protocol_fee1: u64 = ((total_interest1 as u128 * futarchy_authority.revenue_share.interest_bps as u128) / BPS_DENOMINATOR as u128) as u64;
                let lp_share0 = total_interest0 as u64;
                let lp_share1 = total_interest1 as u64;

                // update protocol revenue reserves (tracks extra fees charged to borrowers)
                self.protocol_revenue_reserve0 += protocol_fee0;
                self.protocol_revenue_reserve1 += protocol_fee1;

                // Total amount borrowers owe = interest + protocol_fee (extra fee)
                let total_borrower_cost0 = total_interest0.checked_add(protocol_fee0 as u128).expect("Interest overflow");
                let total_borrower_cost1 = total_interest1.checked_add(protocol_fee1 as u128).expect("Interest overflow");

                // update total debt - includes interest plus protocol fee (extra fee charged to borrowers)
                self.total_debt0 = self.total_debt0
                    .checked_add(u64::try_from(total_borrower_cost0).expect("Interest overflow"))
                    .expect("Total debt0 overflow");
                self.total_debt1 = self.total_debt1
                    .checked_add(u64::try_from(total_borrower_cost1).expect("Interest overflow"))
                    .expect("Total debt1 overflow");

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
                self.reserve0 += lp_share0;
                self.reserve1 += lp_share1;

                emit!(UpdatePairEvent {
                    metadata: EventMetadata::new(Pubkey::default(), pair_key),
                    price0_ema: self.last_price0_ema,
                    price1_ema: self.last_price1_ema,
                    rate0: self.last_rate0,
                    rate1: self.last_rate1,
                    accrued_interest0: total_interest0,
                    accrued_interest1: total_interest1,
                    protocol_revenue_reserve0: self.protocol_revenue_reserve0,
                    protocol_revenue_reserve1: self.protocol_revenue_reserve1,
                    reserve0_after_interest: self.reserve0,
                    reserve1_after_interest: self.reserve1,
                });
            }
            
            self.last_update = current_time;
        }
        
        Ok(())
    }
}

#[macro_export]
macro_rules! generate_gamm_pair_seeds {
    ($pair:expr) => {
        [
            PAIR_SEED_PREFIX,
            $pair.token0.as_ref(),
            $pair.token1.as_ref(),
            $pair.params_hash.as_ref(),
            &[$pair.bump],
        ]
    };
}