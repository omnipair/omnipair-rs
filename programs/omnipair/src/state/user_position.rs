use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::gamm_math::max_borrowable_with_safety;
use crate::errors::ErrorCode;
use super::Pair;

#[account]
pub struct UserPosition {
    // User and pair info
    pub owner: Pubkey,         // who owns this position
    pub pair: Pubkey,          // the pair this position belongs to
    
    // Collateral tracking
    pub collateral0: u64,      // token0 collateral amount
    pub collateral1: u64,      // token1 collateral amount
    
    // Debt tracking
    pub debt0_shares: u64,     // debt shares for token0
    pub debt1_shares: u64,     // debt shares for token1

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
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.pair != Pubkey::default()
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

    /// Get the borrow limit and effective collateral factor in BPS
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns a tuple containing:
    /// - The borrow limit in the debt token
    pub fn get_borrow_limit_and_effective_cf_bps(&self, pair: &Pair, debt_token: &Pubkey) -> (u64, u16) {
        let user_position = &self;

        let (
            user_collateral, 
            collateral_spot_price,
            collateral_ema_price,
            // in token X (debt token)
            pair_debt_reserve,
            pair_total_debt,
        ) = match *debt_token == pair.token0 {
            true => (
                user_position.collateral1,
                pair.spot_price1_nad(),
                pair.ema_price1_nad(),
                pair.reserve0,
                pair.total_debt0,
            ),
            false => (
                user_position.collateral0,
                pair.spot_price0_nad(),
                pair.ema_price0_nad(),
                pair.reserve1,
                pair.total_debt1,
            )
        };

        max_borrowable_with_safety(
            user_collateral,
            collateral_ema_price,
            collateral_spot_price,
            pair_total_debt,
            pair_debt_reserve,
        )
    }

    /// Get the borrow limit in the debt token
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the borrow limit in the debt token
    pub fn get_borrow_limit(&self, pair: &Pair, debt_token: &Pubkey) -> u64 {
        self.get_borrow_limit_and_effective_cf_bps(pair, debt_token).0
    }

    /// Get the effective collateral factor in BPS
    /// 
    /// - `pair`: The pair the user position belongs to
    /// - `debt_token`: The token the user is borrowing
    /// 
    /// Returns the effective collateral factor in BPS
    pub fn get_effective_collateral_factor_bps(&self, pair: &Pair, debt_token: &Pubkey) -> u64 {
        self.get_borrow_limit_and_effective_cf_bps(pair, debt_token).1 as u64
    }

    // debt utilization bps = debt / borrow power
    // borrow power = collateral value * effective collateral factor
    // 0 is safe, > 100% in BPS is unsafe
    pub fn get_debt_utilization_bps(&self, pair: &Pair, token: &Pubkey) -> Result<u64> {
        let is_token0 = token == &pair.token0;
        let debt = match is_token0 {
            true => self.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
            false => self.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        };
        if debt == 0 {
            return Ok(0); // no debt = 0% usage = safe
        }
    
        // NOTE: debt in token0 → collateral is token1
        let borrow_limit = match is_token0 {
            true => self.get_borrow_limit(pair, &pair.token0),
            false => self.get_borrow_limit(pair, &pair.token1),
        };
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