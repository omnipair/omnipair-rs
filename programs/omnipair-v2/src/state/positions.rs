use anchor_lang::prelude::*;

use super::DebtBook;
use crate::{
    errors::ErrorCode,
    utils::market_math::{accrue_fee_liability, active_stake_units},
};

#[account]
#[derive(InitSpace)]
pub struct MarginPosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub available_buffer_share_amount: u64,
    pub staked_claim_token_amount: u64,
    pub staked_buffer_share_amount: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct HedgePosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub hedged_claim_token_amount: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
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
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidMarginPosition);
        require_keys_eq!(self.market, market, ErrorCode::InvalidMarginPosition);
        Ok(())
    }

    pub fn idle_collateral0(&self) -> Result<u64> {
        self.collateral0
            .checked_sub(self.recognized_collateral0_for_debt1)
            .ok_or(ErrorCode::InsufficientRecognizedCollateral.into())
    }

    pub fn idle_collateral1(&self) -> Result<u64> {
        self.collateral1
            .checked_sub(self.recognized_collateral1_for_debt0)
            .ok_or(ErrorCode::InsufficientRecognizedCollateral.into())
    }

    pub fn fixed_debt0(&self, debt_book: &DebtBook) -> Result<u128> {
        DebtBook::shares_to_debt(self.fixed_debt0_shares, debt_book.borrow_index0_nad)
    }

    pub fn fixed_debt1(&self, debt_book: &DebtBook) -> Result<u128> {
        DebtBook::shares_to_debt(self.fixed_debt1_shares, debt_book.borrow_index1_nad)
    }
}

impl StakePosition {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.asset_mint = asset_mint;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default()
            && self.market != Pubkey::default()
            && self.asset_mint != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidStakePosition);
        require_keys_eq!(self.market, market, ErrorCode::InvalidStakePosition);
        require_keys_eq!(self.asset_mint, asset_mint, ErrorCode::InvalidStakePosition);
        Ok(())
    }

    pub fn credit_buffer_share_amount(&mut self, amount: u64) -> Result<()> {
        self.available_buffer_share_amount = self
            .available_buffer_share_amount
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn active_stake_units(&self, buffer_ratio_bps: u16) -> Result<u64> {
        active_stake_units(
            self.staked_claim_token_amount,
            self.staked_buffer_share_amount,
            buffer_ratio_bps,
        )
    }

    pub fn accrue_fees(&mut self, fee_growth_index_nad: u128, buffer_ratio_bps: u16) -> Result<()> {
        let active_units = self.active_stake_units(buffer_ratio_bps)?;
        let accrued_amount = accrue_fee_liability(
            active_units,
            fee_growth_index_nad,
            self.fee_growth_checkpoint_nad,
        )?;
        self.accrued_fee_amount = self
            .accrued_fee_amount
            .checked_add(accrued_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fee_growth_checkpoint_nad = fee_growth_index_nad;
        Ok(())
    }

    pub fn stake(&mut self, claim_amount: u64, buffer_share_amount: u64) -> Result<()> {
        require!(
            claim_amount > 0 && buffer_share_amount > 0,
            ErrorCode::AmountZero
        );
        require_gte!(
            self.available_buffer_share_amount,
            buffer_share_amount,
            ErrorCode::InsufficientBufferShares
        );
        self.available_buffer_share_amount = self
            .available_buffer_share_amount
            .checked_sub(buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.staked_claim_token_amount = self
            .staked_claim_token_amount
            .checked_add(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.staked_buffer_share_amount = self
            .staked_buffer_share_amount
            .checked_add(buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn unstake(&mut self, claim_amount: u64, buffer_share_amount: u64) -> Result<()> {
        require!(
            claim_amount > 0 && buffer_share_amount > 0,
            ErrorCode::AmountZero
        );
        require_gte!(
            self.staked_claim_token_amount,
            claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            self.staked_buffer_share_amount,
            buffer_share_amount,
            ErrorCode::InsufficientBufferShares
        );
        self.staked_claim_token_amount = self
            .staked_claim_token_amount
            .checked_sub(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.staked_buffer_share_amount = self
            .staked_buffer_share_amount
            .checked_sub(buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.available_buffer_share_amount = self
            .available_buffer_share_amount
            .checked_add(buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }
}

impl HedgePosition {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.asset_mint = asset_mint;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default()
            && self.market != Pubkey::default()
            && self.asset_mint != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidHedgePosition);
        require_keys_eq!(self.market, market, ErrorCode::InvalidHedgePosition);
        require_keys_eq!(self.asset_mint, asset_mint, ErrorCode::InvalidHedgePosition);
        Ok(())
    }

    pub fn accrue_fees(&mut self, hedged_fee_growth_index_nad: u128) -> Result<()> {
        let accrued_amount = accrue_fee_liability(
            self.hedged_claim_token_amount,
            hedged_fee_growth_index_nad,
            self.fee_growth_checkpoint_nad,
        )?;
        self.accrued_fee_amount = self
            .accrued_fee_amount
            .checked_add(accrued_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fee_growth_checkpoint_nad = hedged_fee_growth_index_nad;
        Ok(())
    }

    pub fn increase(&mut self, amount: u64) -> Result<()> {
        self.hedged_claim_token_amount = self
            .hedged_claim_token_amount
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn decrease(&mut self, amount: u64) -> Result<()> {
        require_gte!(
            self.hedged_claim_token_amount,
            amount,
            ErrorCode::InvalidHedgePosition
        );
        self.hedged_claim_token_amount = self
            .hedged_claim_token_amount
            .checked_sub(amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }
}
