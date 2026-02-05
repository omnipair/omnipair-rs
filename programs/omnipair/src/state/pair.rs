use anchor_lang::prelude::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::utils::gamm_math::pessimistic_max_debt;
use crate::utils::math::{compute_ema, slots_to_ms, ceil_div};
use crate::state::RateModel;
use crate::events::{UpdatePairEvent, EventMetadata};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct VaultBumps {
    pub reserve0: u8,
    pub reserve1: u8,
    pub collateral0: u8,
    pub collateral1: u8,
}

/// Tracks exponential moving averages (EMAs) for the last observed price.
/// - `symmetric`: standard two-way EMA (exponential price growth and decay)
/// - `directional`: one-way bottom-up asymmetric EMA (exponential price growth, but snaps instantly on price drops)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct LastPriceEMA {
    pub symmetric: u64,
    pub directional: u64,
}

#[account]
#[derive(InitSpace)]
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
    
    // Virtual Reserves (r_virtual = r_cash + r_debt)
    pub reserve0: u64,
    pub reserve1: u64,
    // Cash Reserves (r_cash)
    pub cash_reserve0: u64,
    pub cash_reserve1: u64,
    
    // Price tracking
    pub last_price0_ema: LastPriceEMA,
    pub last_price1_ema: LastPriceEMA,
    pub last_update: u64,
    
    // Rates
    pub last_rate0: u64,
    pub last_rate1: u64,
    
    // Debt tracking (r_debt)
    pub total_debt0: u64,
    pub total_debt1: u64,
    pub total_debt0_shares: u128,
    pub total_debt1_shares: u128,
    
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

    /// Per-pair reduce-only mode - when enabled, blocks borrowing and adding liquidity for this pair
    pub reduce_only: bool,
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
        current_slot: u64,
        params_hash: [u8; 32],
        version: u8,
        bump: u8,
        vault_bumps: VaultBumps,
        initial_rate: u64, // NAD-scaled initial rate from rate model
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
            last_update: current_slot,

            reserve0: 0,
            reserve1: 0,
            cash_reserve0: 0,
            cash_reserve1: 0,
            total_supply: MIN_LIQUIDITY,

            last_price0_ema: LastPriceEMA {
                symmetric: 0,
                directional: 0,
            },
            last_price1_ema: LastPriceEMA {
                symmetric: 0,
                directional: 0,
            },
            last_rate0: initial_rate,
            last_rate1: initial_rate,

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
            reduce_only: false,
        }
    }

    pub fn k(&self) -> u128 {
        self.reserve0 as u128 * self.reserve1 as u128
    }

    pub fn get_collateral_token(&self, debt_token_mint: &Pubkey) -> Pubkey {
       self.get_token_y(debt_token_mint)
    }

    pub fn get_debt_token(&self, collateral_token_mint: &Pubkey) -> Pubkey {
        self.get_token_y(collateral_token_mint)
    }

    pub fn get_token_y(&self, token_x: &Pubkey) -> Pubkey {
        match *token_x == self.token0 {
            true => self.token1,
            false => self.token0,
        }
    }

    pub fn spot_price0_nad(&self) -> u64 {
        match self.reserve0 {
            0 => 0,
            _ => {
                let price = (self.reserve1 as u128 * NAD as u128) / self.reserve0 as u128;
                u64::try_from(price).unwrap_or(u64::MAX)
            }
        }
    }

    pub fn spot_price1_nad(&self) -> u64 {
        match self.reserve1 {
            0 => 0,
            _ => {
                let price = (self.reserve0 as u128 * NAD as u128) / self.reserve1 as u128;
                u64::try_from(price).unwrap_or(u64::MAX)
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
                self.last_price0_ema.symmetric, 
                self.last_update, 
                spot_price, 
                self.half_life
            )
        }
    }

    pub fn ema_price1_nad(&self) -> u64 {
        if self.reserve1 == 0 {
            0
        } else {
            let spot_price = self.spot_price1_nad();
            compute_ema(
                self.last_price1_ema.symmetric, 
                self.last_update, 
                spot_price, 
                self.half_life
            )
        }
    }

    pub fn directional_ema_price0_nad(&self) -> u64 {
        if self.reserve0 == 0 {
            0
        } else {
            let spot_price = self.spot_price0_nad();
            compute_ema(
                self.last_price0_ema.directional, 
                self.last_update, 
                spot_price, 
                DIRECTIONAL_EMA_HALF_LIFE_MS
            )
        }
    }

    pub fn directional_ema_price1_nad(&self) -> u64 {
        if self.reserve1 == 0 {
            0
        } else {
            let spot_price = self.spot_price1_nad();
            compute_ema(
                self.last_price1_ema.directional, 
                self.last_update, 
                spot_price, 
                DIRECTIONAL_EMA_HALF_LIFE_MS
            )
        }
    }

    pub fn get_rates(&self, rate_model: &Account<RateModel>) -> Result<(u64, u64)> {
        let current_slot = Clock::get()?.slot;
        let time_elapsed = slots_to_ms(self.last_update, current_slot).unwrap_or(0);

        let util0 = match self.reserve0 {
            0 => 0,
            _ => {
                let util = (self.total_debt0 as u128 * NAD as u128) / self.reserve0 as u128;
                u64::try_from(util).unwrap_or(u64::MAX)
            }
        };
        let util1 = match self.reserve1 {
            0 => 0,
            _ => {
                let util = (self.total_debt1 as u128 * NAD as u128) / self.reserve1 as u128;
                u64::try_from(util).unwrap_or(u64::MAX)
            }
        };

        Ok((
            rate_model.calculate_rate(self.last_rate0, time_elapsed, util0).0, 
            rate_model.calculate_rate(self.last_rate1, time_elapsed, util1).0
        ))
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
    /// 
    /// Computes collateral limits using directional and symmetric EMAs:
    /// directional EMA replaces raw spot price in divergence logic,
    /// enabling one_way_ema / two_way_ema comparisons rather than raw_spot/ema. 
    /// This provides more front-running resistance by capturing quick price drops, while still smoothing upward movements.
    pub fn get_max_debt_and_cf_bps_for_collateral(&self, pair: &Pair, collateral_token: &Pubkey, collateral_amount: u64) -> Result<(u64, u16, u16)> {
        let (
            collateral_ema_price,
            collateral_directional_ema_price,
            collateral_amm_reserve,
            debt_amm_reserve,
            debt_total,
            
        ) = match collateral_token == &pair.token0 {
            true => (pair.ema_price0_nad(), pair.directional_ema_price0_nad(), pair.reserve0, pair.reserve1, pair.total_debt1),
            false => (pair.ema_price1_nad(), pair.directional_ema_price1_nad(), pair.reserve1, pair.reserve0, pair.total_debt0),
        };

        pessimistic_max_debt(
            collateral_amount,
            collateral_ema_price,
            collateral_directional_ema_price, // will jump down immediately to a new low, but rise gradually in ~ 50 slots (~20 seconds)
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
        let current_slot = Clock::get()?.slot;
        let spot_price0 = self.spot_price0_nad();
        let spot_price1 = self.spot_price1_nad();

        // Always update directional EMAs, even within the same slot
        self.last_price0_ema.directional = self.last_price0_ema.directional.min(spot_price0);
        self.last_price1_ema.directional = self.last_price1_ema.directional.min(spot_price1);
        
        if current_slot > self.last_update {
            // Update oracles
            let time_elapsed = slots_to_ms(self.last_update, current_slot).unwrap();
            if time_elapsed > 0 {
                // Update price EMAs
                self.last_price0_ema.symmetric = compute_ema(
                    self.last_price0_ema.symmetric,
                    self.last_update,
                    spot_price0,
                    self.half_life
                );
                self.last_price1_ema.symmetric = compute_ema(
                    self.last_price1_ema.symmetric,
                    self.last_update,
                    spot_price1,
                    self.half_life
                );

                let new_ema0 = compute_ema(
                    self.last_price0_ema.directional,
                    self.last_update,
                    spot_price0,
                    DIRECTIONAL_EMA_HALF_LIFE_MS
                );
                self.last_price0_ema.directional = if spot_price0 < new_ema0 { spot_price0 } else { new_ema0 };
                
                let new_ema1 = compute_ema(
                    self.last_price1_ema.directional,
                    self.last_update,
                    spot_price1,
                    DIRECTIONAL_EMA_HALF_LIFE_MS
                );
                self.last_price1_ema.directional = if spot_price1 < new_ema1 { spot_price1 } else { new_ema1 };
                
                // Calculate utilization rates
                let util0 = match self.reserve0 {
                    0 => 0,
                    _ => {
                        let util = (self.total_debt0 as u128 * NAD as u128) / self.reserve0 as u128;
                        u64::try_from(util).unwrap_or(u64::MAX)
                    }
                };
                let util1 = match self.reserve1 {
                    0 => 0,
                    _ => {
                        let util = (self.total_debt1 as u128 * NAD as u128) / self.reserve1 as u128;
                        u64::try_from(util).unwrap_or(u64::MAX)
                    }
                };
                
                // Calculate new rates
                let (new_rate0, integral0) = rate_model.calculate_rate(
                    self.last_rate0, 
                    time_elapsed, 
                    util0
                );
                let (new_rate1, integral1) = rate_model.calculate_rate(
                    self.last_rate1, 
                    time_elapsed, 
                    util1
                );
                
                // Update rates
                self.last_rate0 = new_rate0;
                self.last_rate1 = new_rate1;
                
                // Calculate and apply interest
                let total_interest0 = ceil_div(self.total_debt0 as u128 * integral0 as u128, NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?;
                let total_interest1 = ceil_div(self.total_debt1 as u128 * integral1 as u128, NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?;

                // Calculate protocol fee as an extra fee on top of interest (not a share of interest)
                // Borrowers pay: interest + protocol_fee
                // LPs receive: interest (full amount)
                // Protocol receives: protocol_fee (extra fee charged to borrowers)
                let protocol_fee0: u64 = u64::try_from(
                    (total_interest0 * futarchy_authority.revenue_share.interest_bps as u128) / BPS_DENOMINATOR as u128
                ).unwrap_or(u64::MAX);
                let protocol_fee1: u64 = u64::try_from(
                    (total_interest1 * futarchy_authority.revenue_share.interest_bps as u128) / BPS_DENOMINATOR as u128
                ).unwrap_or(u64::MAX);
                let lp_share0 = u64::try_from(total_interest0).unwrap_or(u64::MAX);
                let lp_share1 = u64::try_from(total_interest1).unwrap_or(u64::MAX);

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

                // The change in virtual reserves (ΔV) depends on accounted cash availability (r_cash, where r_cash >= 0):
                // 1. Always add lp_share to virtual reserves.
                // 2. Only add the "uncovered" portion of protocol_fee to virtual reserves.
                //
                // This ensures the state ΔR_virtual = ΔR_cash + ΔR_debt holds, where:
                // L: r_virtual + lp_share + (protocol_fee - cash_covered_fee)
                // R: (r_cash - cash_covered_fee) + (r_debt + lp_share + protocol_fee)
                // where:
                // cash_covered_fee = min(protocol_fee, cash_reserve)
                // Realized protocol fees is the reserve token balance - accounted cash reserve (r_actual - r_cash)

                // 1. Calculate the portion of the fee covered by cash reserves (r_cash)
                let cash_covered_fee0 = protocol_fee0.min(self.cash_reserve0);
                let cash_covered_fee1 = protocol_fee1.min(self.cash_reserve1);

                // 2. Update virtual reserves
                // ΔV = lp_share + (protocol_fee - cash_covered_fee)
                self.reserve0 = self.reserve0.saturating_add(lp_share0 + (protocol_fee0 - cash_covered_fee0)); // won't underflow because protocol_fee0 <= cash_covered_fee
                self.reserve1 = self.reserve1.saturating_add(lp_share1 + (protocol_fee1 - cash_covered_fee1));

                // 3. Update physical cash reserves
                // Cash reserves are reduced by the amount we can afford to take (as r_cash can't go below zero), 
                // Any uncovered fee remains in virtual reserves, so LP's gets the claim on the uncovered fee
                self.cash_reserve0 -= cash_covered_fee0; // won't underflow because cash_covered_fee <= cash_reserve
                self.cash_reserve1 -= cash_covered_fee1;

                emit!(UpdatePairEvent {
                    metadata: EventMetadata::new(Pubkey::default(), pair_key),
                    price0_ema: self.last_price0_ema.symmetric,
                    price1_ema: self.last_price1_ema.symmetric,
                    rate0: self.last_rate0,
                    rate1: self.last_rate1,
                    accrued_interest0: total_interest0,
                    accrued_interest1: total_interest1,
                    cash_reserve0: self.cash_reserve0, 
                    cash_reserve1: self.cash_reserve1,
                    reserve0_after_interest: self.reserve0,
                    reserve1_after_interest: self.reserve1,
                });
            }
            
            self.last_update = current_slot;
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