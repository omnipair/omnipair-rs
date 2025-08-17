use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::gamm_math::{pessimistic_max_debt, pessimistic_min_collateral};
use crate::errors::ErrorCode;
use super::Pair;
use std::cmp::max;

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

    /// Get the liquidation collateral factor in BPS for a given debt token
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the max of the pessimistic collateral factor in BPS and the applied min. cf in BPS
    pub fn get_liquidation_cf_bps(&self, pair: &Pair, debt_token: &Pubkey) -> u16 {
        match debt_token == &pair.token1 {
            true => {
                let cf_bps = self.get_pessimistic_collateral_factor_bps(pair, debt_token);
                let min_cf_bps = self.collateral0_applied_min_cf_bps;
                max(cf_bps, min_cf_bps)
            },
            false => {
                let cf_bps = self.get_pessimistic_collateral_factor_bps(pair, debt_token);
                let min_cf_bps = self.collateral1_applied_min_cf_bps;
                max(cf_bps, min_cf_bps)
            }
        }        
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
                    let shares = (amount as u128)
                        .checked_mul(pair.total_debt0_shares as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt0 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                        .try_into()
                        .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                    pair.total_debt0_shares = pair.total_debt0_shares.saturating_add(shares);
                    self.debt0_shares = self.debt0_shares.saturating_add(shares);
                }
                pair.total_debt0 = pair.total_debt0.saturating_add(amount);
            }
            false => {
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1_shares = amount;
                    self.debt1_shares = amount;
                } else {
                    let shares = (amount as u128)
                        .checked_mul(pair.total_debt1_shares as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt1 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                        .try_into()
                        .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                    pair.total_debt1_shares = pair.total_debt1_shares.saturating_add(shares);
                    self.debt1_shares = self.debt1_shares.saturating_add(shares);
                }
                pair.total_debt1 = pair.total_debt1.saturating_add(amount);
            }
        }
        Ok(())
    }
    

    pub fn decrease_debt(&mut self, pair: &mut Pair, debt_token: &Pubkey, amount: u64) -> Result<()> {
        msg!("decrease_debt: {}", amount);
        match *debt_token == pair.token0 {
            true => {
                let shares = (amount as u128)
                    .checked_mul(pair.total_debt0_shares as u128)
                    .ok_or(ErrorCode::DebtShareMathOverflow)?
                    .checked_div(pair.total_debt0 as u128)
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                    .try_into()
                    .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                self.debt0_shares = self.debt0_shares.saturating_sub(shares);
                pair.total_debt0_shares = pair.total_debt0_shares.saturating_sub(shares);
                pair.total_debt0 = pair.total_debt0.saturating_sub(amount);
            }
            false => {
                let shares = (amount as u128)
                    .checked_mul(pair.total_debt1_shares as u128)
                    .ok_or(ErrorCode::DebtShareMathOverflow)?
                    .checked_div(pair.total_debt1 as u128)
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                    .try_into()
                    .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?;
                self.debt1_shares = self.debt1_shares.saturating_sub(shares);
                pair.total_debt1_shares = pair.total_debt1_shares.saturating_sub(shares);
                pair.total_debt1 = pair.total_debt1.saturating_sub(amount);
            }
        }
        Ok(())
    }

    pub fn calculate_debt0(&self, total_debt0: u64, total_debt0_shares: u64) -> Result<u64> {
        msg!("total_debt0: {}, total_debt0_shares: {}", total_debt0, total_debt0_shares);
        msg!("debt0_shares: {}", self.debt0_shares.to_string());
        match total_debt0_shares {
            0 => Ok(0),
            _ => Ok((self.debt0_shares as u128)
                .checked_mul(total_debt0 as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(total_debt0_shares as u128)
                .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                .try_into()
                .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?)
        }
    }

    pub fn calculate_debt1(&self, total_debt1: u64, total_debt1_shares: u64) -> Result<u64> {
        msg!("total_debt0: {}, total_debt0_shares: {}", total_debt1, total_debt1_shares);
        msg!("debt0_shares: {}", self.debt1_shares.to_string());
        match total_debt1_shares {
            0 => Ok(0),
            _ => Ok((self.debt1_shares as u128)
                .checked_mul(total_debt1 as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(total_debt1_shares as u128)
                .ok_or(ErrorCode::DebtShareDivisionOverflow)?
                .try_into()
                .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?)
        }
    }

    /// Get the borrow limit and pessimistic collateral factor in BPS for user deposited collateral
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns a tuple containing:
    /// - The borrow limit in the debt token
    pub fn get_borrow_limit_and_cf_bps_for_collateral(&self, pair: &Pair, debt_token: &Pubkey) -> (u64, u16) {
        let user_position = &self;

        let (
            user_collateral, 
            collateral_ema_price,
            collateral_spot_price,
            // in token X (debt token)
            debt_amm_reserve,
        ) = match *debt_token == pair.token0 {
            true => (
                user_position.collateral1,
                pair.ema_price1_nad(),
                pair.spot_price1_nad(),
                pair.reserve0,
            ),
            false => (
                user_position.collateral0,
                pair.ema_price0_nad(),
                pair.spot_price0_nad(),
                pair.reserve1,
            )
        };

        pessimistic_max_debt(
            user_collateral,
            collateral_ema_price,
            collateral_spot_price,
            debt_amm_reserve,
        ).unwrap()
    }

    /// Get the borrow limit in the debt token for user deposited collateral
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the borrow limit in the debt token
    pub fn get_borrow_limit(&self, pair: &Pair, debt_token: &Pubkey) -> u64 {
        self.get_borrow_limit_and_cf_bps_for_collateral(pair, debt_token).0
    }

    /// Get the pessimistic collateral factor in BPS for user deposited collateral
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the pessimistic collateral factor in BPS
    pub fn get_pessimistic_collateral_factor_bps(&self, pair: &Pair, debt_token: &Pubkey) -> u16 {
        self.get_borrow_limit_and_cf_bps_for_collateral(pair, debt_token).1
    }

    /// Get the minimum collateral and pessimistic collateral factor in BPS for a given debt amount
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_amount`: The amount of debt the user is borrowing
    /// 
    /// Returns a tuple containing:
    /// - The minimum collateral required to avoid liquidation
    pub fn get_min_collateral_and_cf_bps_for_debt(&self, pair: &Pair, debt_amount: u64) -> Result<(u64, u16)> {
        pessimistic_min_collateral(
            debt_amount,
            pair.ema_price0_nad(),
            pair.spot_price0_nad(),
            pair.reserve0,
        ).map_err(|error| error.into())
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
        Ok((collateral_value as u128)
            .checked_mul(applied_min_cf_bps as u128)
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
        let applied_min_cf_bps = self.get_liquidation_cf_bps(pair, debt_token);
        let borrow_limit = self.get_remaining_borrow_limit(pair, debt_token, applied_min_cf_bps)?;
        
        
        if borrow_limit == 0 {
            return Ok(u64::MAX); // zero borrow limit, user should be liquidated
        }
    
        Ok(debt
            .saturating_mul(BPS_DENOMINATOR as u64)
            .checked_div(borrow_limit)
            .ok_or(ErrorCode::DebtUtilizationOverflow)?)
    }
    
}

#[macro_export]
macro_rules! generate_user_position_seeds {
    ($position:expr) => {{
        &[
            USER_POSITION_SEED_PREFIX,
            $position.pair.as_ref(),
            $position.owner.as_ref(),
            &[$position.bump],
        ]
    }};
} 