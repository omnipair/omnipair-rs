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
    pub base_collateral: u64,
    pub quote_collateral: u64,
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub fixed_base_debt_shares: u128,
    pub fixed_quote_debt_shares: u128,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    /// Non-transferable junior buffer accounting credited by add_liquidity.
    /// Removing claim-token principal does not release these units; they remain
    /// available to match with claim tokens for future fee eligibility.
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

    pub fn fixed_base_debt(&self, debt_book: &DebtBook) -> Result<u128> {
        DebtBook::shares_to_debt(self.fixed_base_debt_shares, debt_book.base_borrow_index_nad)
    }

    pub fn fixed_quote_debt(&self, debt_book: &DebtBook) -> Result<u128> {
        DebtBook::shares_to_debt(
            self.fixed_quote_debt_shares,
            debt_book.quote_borrow_index_nad,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constants::NAD, state::MarketSide};

    fn stake_position() -> StakePosition {
        StakePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            available_buffer_share_amount: 0,
            staked_claim_token_amount: 0,
            staked_buffer_share_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    fn hedge_position() -> HedgePosition {
        HedgePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_token_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    #[test]
    fn stake_position_accrues_checkpointed_non_compounding_fees() {
        let mut position = stake_position();
        position.credit_buffer_share_amount(200_000).unwrap();
        position.stake(800_000, 200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 3 * NAD as u128);

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
    }

    #[test]
    fn hedge_position_tracks_one_to_one_nav_without_stake_rights() {
        let mut market_side = MarketSide::default();
        let mut position = hedge_position();

        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_add(500_000)
            .unwrap();
        position.increase(500_000).unwrap();

        assert_eq!(position.hedged_claim_token_amount, 500_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            500_000
        );
        assert_eq!(market_side.claim_token_ledger.staked_claim_token_supply, 0);
        assert_eq!(market_side.buffer_ledger.staked_buffer_share_amount, 0);

        position.decrease(125_000).unwrap();
        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_sub(125_000)
            .unwrap();
        assert_eq!(position.hedged_claim_token_amount, 375_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            375_000
        );
    }

    #[test]
    fn hedge_position_accrues_checkpointed_routed_fees() {
        let mut position = hedge_position();
        position.increase(200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 4 * NAD as u128);

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
    }
}
