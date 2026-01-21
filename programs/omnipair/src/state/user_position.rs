use anchor_lang::prelude::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::utils::math::ceil_div;
use super::Pair;
use std::cmp::max;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtDecreaseReason {
    /// Repayment: calculate shares from amount
    Repayment,
    /// WriteOff: expects exact debt shares to be written off
    WriteOff(u64),
}

#[account]
pub struct UserPosition {
    // User and pair info
    pub owner: Pubkey,             // who owns this position
    pub pair: Pubkey,              // the pair this position belongs to
    pub collateral0_applied_min_cf_bps: u16, // applied min. cf for borrowing token1 using token0 as collateral
    pub collateral1_applied_min_cf_bps: u16, // applied min. cf for borrowing token0 using token1 as collateral
    
    // Collateral tracking
    pub collateral0: u64,          // token0 collateral amount
    pub collateral1: u64,          // token1 collateral amount
    
    // Debt tracking
    pub debt0_shares: u64,         // debt shares for token0
    pub debt1_shares: u64,         // debt shares for token1

    // PDA bump
    pub bump: u8,
}

impl UserPosition {
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        pair: Pubkey,
        bump: u8,
    ) -> Result<()> {
        self.owner = owner;
        self.pair = pair;
        self.bump = bump;
        self.collateral0_applied_min_cf_bps = 0; // Start with dynamic CF
        self.collateral1_applied_min_cf_bps = 0; // Start with dynamic CF
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.pair != Pubkey::default()
    }

    /// Set applied min. cf for a specific debt token
    pub fn set_applied_min_cf_for_debt_token(&mut self, debt_token: &Pubkey, pair: &Pair, cf_bps: u16) {
        if *debt_token == pair.token1 {
            self.collateral0_applied_min_cf_bps = cf_bps;
        } else {
            self.collateral1_applied_min_cf_bps = cf_bps;
        }
    }

    pub fn increase_debt(&mut self, pair: &mut Pair, debt_token: &Pubkey, amount: u64) -> Result<()> {
        match *debt_token == pair.token0 {
            true => {
                if pair.total_debt0_shares == 0 {
                    pair.total_debt0_shares = amount;
                    self.debt0_shares = amount;
                } else {
                    let shares = ceil_div(
                        (amount as u128)
                            .checked_mul(pair.total_debt0_shares as u128)
                            .ok_or(ErrorCode::DebtShareMathOverflow)?,
                        pair.total_debt0 as u128
                    )
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                    .try_into()
                    .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                    pair.total_debt0_shares = pair.total_debt0_shares.saturating_add(shares);
                    self.debt0_shares = self.debt0_shares.saturating_add(shares);
                }
                pair.total_debt0 = pair.total_debt0.saturating_add(amount);
                pair.cash_reserve0 = pair.cash_reserve0.checked_sub(amount).ok_or(ErrorCode::CashReserveUnderflow)?;
            }
            false => {
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1_shares = amount;
                    self.debt1_shares = amount;
                } else {
                    let shares = ceil_div(
                        (amount as u128)
                            .checked_mul(pair.total_debt1_shares as u128)
                            .ok_or(ErrorCode::DebtShareMathOverflow)?,
                        pair.total_debt1 as u128
                    )
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                    .try_into()
                    .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                    pair.total_debt1_shares = pair.total_debt1_shares.saturating_add(shares);
                    self.debt1_shares = self.debt1_shares.saturating_add(shares);
                }
                pair.total_debt1 = pair.total_debt1.saturating_add(amount);
                pair.cash_reserve1 = pair.cash_reserve1.checked_sub(amount).ok_or(ErrorCode::CashReserveUnderflow)?;
            }
        }
        Ok(())
    }
    

    /// Decrease debt. Two modes based on reason:
    /// - Repayment: calculates shares from amount (floor div), adds to cash_reserve
    /// - WriteOff(exact_shares): uses exact shares to avoid rounding edge cases, reduces virtual reserve (debt forgiven during liquidation)
    // Invariants: 
    // 1. x_virtual * y_virtual = k (Constant product invariant)
    // 2. r_virtual >= r_debt (Solvency invariant)
    // with a state transition: ΔR_virtual = ΔR_cash + ΔR_debt
    //
    // I. during solvency 
    //   1. debt repayment: r_virtual (constant) = (r_cash + amount) + (r_debt - amount) [debt reduced, cash reserve increased]
    //   2. during liquidation: 
    //      a. x_virtual (-written_off_debt) = x_cash (constant) + (x_debt - written_off_debt)
    //      b. y_virtual + (collateral_seized_amount) = y_cash + (collateral_seized_amount) + y_debt (constant)
    //      c. x_virtual * y_virtual >= last_k
    //      where collateral_seized amount value > reduced_debt value
    // II. during insolvency: 
    //   same as (2) but with collateral_seized amount value < reduced_debt value so k_new < k_old
    //   reduced k means bad debt is accrued and socialized via LP math
    // any surplus in repayment (r_virtual - r_cash) is protocol fee
    pub fn decrease_debt(&mut self, pair: &mut Pair, debt_token: &Pubkey, amount: u64, reason: DebtDecreaseReason) -> Result<()> {
        match *debt_token == pair.token0 {
            true => {
                let shares = match reason {
                    DebtDecreaseReason::WriteOff(exact_shares) => exact_shares.min(self.debt0_shares),
                    DebtDecreaseReason::Repayment => (amount as u128)
                        .checked_mul(pair.total_debt0_shares as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt0 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                        .try_into()
                        .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?
                };
                self.debt0_shares = self.debt0_shares.saturating_sub(shares);
                pair.total_debt0_shares = pair.total_debt0_shares.saturating_sub(shares);
                pair.total_debt0 = pair.total_debt0.saturating_sub(amount);
                // if debt is repaid, add the amount to the cash reserve (avoid adding to cash reserve if debt is written off)
                match reason {
                    DebtDecreaseReason::Repayment => pair.cash_reserve0 = pair.cash_reserve0.saturating_add(amount),
                    // r_virtual can't reach zero during write off
                    DebtDecreaseReason::WriteOff(_) => pair.reserve0 = pair.reserve0.checked_sub(amount).unwrap_or(1),
                };
            }
            false => {
                let shares = match reason {
                    DebtDecreaseReason::WriteOff(exact_shares) => exact_shares.min(self.debt1_shares),
                    DebtDecreaseReason::Repayment => (amount as u128)
                        .checked_mul(pair.total_debt1_shares as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt1 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                        .try_into()
                        .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?
                };
                self.debt1_shares = self.debt1_shares.saturating_sub(shares);
                pair.total_debt1_shares = pair.total_debt1_shares.saturating_sub(shares);
                pair.total_debt1 = pair.total_debt1.saturating_sub(amount);
                match reason {
                    DebtDecreaseReason::Repayment => pair.cash_reserve1 = pair.cash_reserve1.saturating_add(amount),
                    DebtDecreaseReason::WriteOff(_) => pair.reserve1 = pair.reserve1.checked_sub(amount).unwrap_or(1),
                };
            }
        }
        Ok(())
    }

    pub fn calculate_debt0(&self, total_debt0: u64, total_debt0_shares: u64) -> Result<u64> {
        match total_debt0_shares {
            0 => Ok(0),
            _ => Ok(ceil_div(
                (self.debt0_shares as u128)
                    .checked_mul(total_debt0 as u128)
                    .ok_or(ErrorCode::DebtMathOverflow)?,
                total_debt0_shares as u128
            )
            .ok_or(ErrorCode::DebtShareDivisionOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?)
        }
    }

    pub fn calculate_debt1(&self, total_debt1: u64, total_debt1_shares: u64) -> Result<u64> {
        match total_debt1_shares {
            0 => Ok(0),
            _ => Ok(ceil_div(
                (self.debt1_shares as u128)
                    .checked_mul(total_debt1 as u128)
                    .ok_or(ErrorCode::DebtMathOverflow)?,
                total_debt1_shares as u128
            )
            .ok_or(ErrorCode::DebtShareDivisionOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?)
        }
    }


    pub fn get_remaining_borrow_limit(&self, pair: &Pair, debt_token: &Pubkey, applied_min_cf_bps: u16) -> Result<u64> {
        let is_token0 = debt_token == &pair.token0;

        // Calculate borrow limit using fixed CF
        let collateral_value = match is_token0 {
            true => (self.collateral1 as u128)
                .checked_mul(pair.ema_price1_nad() as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(NAD as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?,
            false => (self.collateral0 as u128)
                .checked_mul(pair.ema_price0_nad() as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(NAD as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?,
        };
        
        // Apply LTV buffer to the CF: reduce borrow limit by LTV_BUFFER_BPS to create a buffer before liquidation
        let cf_with_buffer = (applied_min_cf_bps as u128)
            .saturating_mul((BPS_DENOMINATOR - LTV_BUFFER_BPS) as u128)
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        Ok((collateral_value as u128)
            .checked_mul(cf_with_buffer)
            .ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::DebtMathOverflow)?)
    }

    /// Get the debt utilization in BPS (debt / borrow power)
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the debt utilization in BPS
    pub fn get_debt_utilization_bps(&self, pair: &Pair, debt_token: &Pubkey) -> Result<u64> {
        let is_token0 = debt_token == &pair.token0;
        let debt = match is_token0 {
            true => self.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
            false => self.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        };
        if debt == 0 {
            return Ok(0); // no debt = 0% usage = safe
        }
    
        // NOTE: debt in token0 → collateral is token1
        // Use maximum allowed collateral factor (not liquidation CF) for accurate debt utilization
        let max_allowed_cf_bps = self.get_max_allowed_cf_bps(pair, debt_token)?;
        let borrow_limit = self.get_remaining_borrow_limit(pair, debt_token, max_allowed_cf_bps)?;
        
        
        if borrow_limit == 0 {
            return Ok(u64::MAX); // zero borrow limit, user should be liquidated
        }
    
        Ok(debt
            .saturating_mul(BPS_DENOMINATOR as u64)
            .checked_div(borrow_limit)
            .ok_or(ErrorCode::DebtUtilizationOverflow)?)
    }

    /// Get the maximum allowed collateral factor in BPS for a given debt token
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the max of the maximum allowed collateral factor in BPS and the applied min. cf in BPS
    pub fn get_max_allowed_cf_bps(&self, pair: &Pair, debt_token: &Pubkey) -> Result<u16> {
        match debt_token == &pair.token1 {
            true => {
                let cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(pair, &pair.token0, self.collateral0)?.1;
                let min_cf_bps = self.collateral0_applied_min_cf_bps;
                Ok(max(cf_bps, min_cf_bps))
            },
            false => {
                let cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(pair, &pair.token1, self.collateral1)?.1;
                let min_cf_bps = self.collateral1_applied_min_cf_bps;
                Ok(max(cf_bps, min_cf_bps))
            }
        }        
    }

        /// Get the liquidation collateral factor in BPS for a given debt token
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the max of the pessimistic collateral factor in BPS and the applied min. cf in BPS
    pub fn get_liquidation_cf_bps(&self, pair: &Pair, debt_token: &Pubkey) -> Result<u16> {
        match debt_token == &pair.token1 {
            true => {
                let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(pair, &pair.token0, self.collateral0)?;
                let min_cf_bps = self.collateral0_applied_min_cf_bps;
                Ok(max(liquidation_cf_bps, min_cf_bps))
            },
            false => {
                let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(pair, &pair.token1, self.collateral1)?;
                let min_cf_bps = self.collateral1_applied_min_cf_bps;
                Ok(max(liquidation_cf_bps, min_cf_bps))
            }
        }        
    }

    /// Returns the NAD-scaled liquidation price of the *collateral* (in debt token units per 1 collateral token).
    /// If borrowing token0, collateral is token1 and we return price(token1 in token0) in NAD units.
    /// If borrowing token1, collateral is token0 and we return price(token0 in token1) in NAD units.
    /// 
    /// Edge cases:
    /// - If debt == 0: returns 0 (not liquidatable).
    /// - If no collateral or CF == 0: returns u64::MAX (immediately unsafe).
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the NAD-scaled liquidation price of the collateral in debt token units per 1 collateral token
    /// Possible outputs:
    /// - u64::MAX: position is immediately unsafe (no collateral or CF == 0)
    /// - u64: NAD-scaled liquidation price
    /// - 0: no liquidation price (no debt)
    /// 
    /// Note:
    /// - Finite(u64): NAD-scaled liquidation price
    /// - Infinite: den == 0 && debt > 0
    /// - NotApplicable: debt == 0 → return current EMA (no liquidation price)

    pub fn get_liquidation_price(&self, pair: &Pair, debt_token: &Pubkey) -> Result<u64> {
        let is_token0_debt = debt_token == &pair.token0;
    
        // raw on-chain amounts (smallest units)
        let (collateral_amount, debt_amount, collateral_decimals, debt_decimals) = if is_token0_debt {
            // borrowing token0 → collateral is token1
            (self.collateral1,
             self.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
             pair.token1_decimals as i32,
             pair.token0_decimals as i32)
        } else {
            // borrowing token1 → collateral is token0
            (self.collateral0,
             self.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
             pair.token0_decimals as i32,
             pair.token1_decimals as i32)
        };
    
        // Not applicable: no debt
        if debt_amount == 0 { return Ok(0); }
    
        let cf_bps = self.get_liquidation_cf_bps(pair, debt_token)? as u128;
        if collateral_amount == 0 || cf_bps == 0 { return Ok(u64::MAX); }
    
        // Adjust for different decimals so price is "per 1 collateral token"
        // P* (NAD) = ceil( debt * 10^{collateral_decimals} * NAD * BPS / (collateral_amount * 10^{debt_decimals} * CF_BPS) )
        let dec_diff = collateral_decimals - debt_decimals; // can be negative
        let (num_dec_mul, den_dec_mul): (u128, u128) = if dec_diff >= 0 {
            (10u128.pow(dec_diff as u32), 1)
        } else {
            (1, 10u128.pow((-dec_diff) as u32))
        };
    
        let num = (debt_amount as u128)
            .saturating_mul(num_dec_mul)
            .saturating_mul(NAD as u128)
            .saturating_mul(BPS_DENOMINATOR as u128);
    
        let den = (collateral_amount as u128)
            .saturating_mul(den_dec_mul)
            .saturating_mul(cf_bps);
    
        if den == 0 { return Ok(u64::MAX); }
    
        let p_star_nad = num
            .saturating_add(den.saturating_sub(1))
            .checked_div(den)
            .unwrap_or(u128::MAX);
    
        Ok(p_star_nad.min(u64::MAX as u128) as u64)
    }
    
}

#[macro_export]
macro_rules! generate_user_position_seeds {
    ($position:expr) => {
        [
            USER_POSITION_SEED_PREFIX,
            $position.pair.as_ref(),
            $position.owner.as_ref(),
            &[$position.bump],
        ]
    };
} 