use anchor_lang::prelude::*;

use super::Debt;
use crate::errors::ErrorCode;

#[account]
#[derive(InitSpace)]
pub struct MarginPosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub base_collateral: u64,
    pub quote_collateral: u64,
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub fixed_base_shares: u128,
    pub fixed_quote_shares: u128,
    pub bump: u8,
}

impl MarginPosition {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.market != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidPositionMarket);
        require_keys_eq!(self.market, market, ErrorCode::InvalidPositionMarket);
        Ok(())
    }

    pub fn idle_base_collateral(&self) -> Result<u64> {
        self.base_collateral
            .checked_sub(self.recognized_base_collateral_for_quote_debt)
            .ok_or(ErrorCode::InsufficientRecognizedCollateral.into())
    }

    pub fn idle_quote_collateral(&self) -> Result<u64> {
        self.quote_collateral
            .checked_sub(self.recognized_quote_collateral_for_base_debt)
            .ok_or(ErrorCode::InsufficientRecognizedCollateral.into())
    }

    pub fn fixed_base_debt(&self, debt: &Debt) -> Result<u128> {
        Debt::shares_to_debt(self.fixed_base_shares, debt.base_borrow_index_nad)
    }

    pub fn fixed_quote_debt(&self, debt: &Debt) -> Result<u128> {
        Debt::shares_to_debt(self.fixed_quote_shares, debt.quote_borrow_index_nad)
    }
}
