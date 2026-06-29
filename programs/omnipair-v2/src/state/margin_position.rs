use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::state::market::{Debt, MarketAsset};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollateralReceipt {
    pub collateral_credit: u64,
    pub collateral_debit: u64,
    pub base_collateral: u64,
    pub quote_collateral: u64,
}

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
    pub risk_epoch: u64,
    pub bump: u8,
}

impl MarginPosition {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.risk_epoch = 0;
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

    pub fn record_risk_update(&mut self) -> Result<()> {
        self.risk_epoch = self
            .risk_epoch
            .checked_add(1)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn deposit_collateral(
        &mut self,
        market_asset: MarketAsset,
        collateral_credit: u64,
    ) -> Result<CollateralReceipt> {
        require!(collateral_credit > 0, ErrorCode::AmountZero);
        match market_asset {
            MarketAsset::Base => {
                self.base_collateral = self
                    .base_collateral
                    .checked_add(collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                self.quote_collateral = self
                    .quote_collateral
                    .checked_add(collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        self.record_risk_update()?;

        Ok(CollateralReceipt {
            collateral_credit,
            collateral_debit: 0,
            base_collateral: self.base_collateral,
            quote_collateral: self.quote_collateral,
        })
    }
}
