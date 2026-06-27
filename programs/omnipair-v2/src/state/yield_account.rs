use anchor_lang::prelude::*;

use super::accrue_fee_liability;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum YieldTokenKind {
    Ylp,
    Hlp,
}

impl YieldTokenKind {
    pub fn code(self) -> u8 {
        match self {
            Self::Ylp => 0,
            Self::Hlp => 1,
        }
    }
}

#[account]
#[derive(InitSpace)]
pub struct YieldAccount {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub token_kind: u8,
    pub recipient: Pubkey,
    pub swap_fee_checkpoint_nad: u128,
    pub interest_checkpoint_nad: u128,
    pub accrued_swap_fee_amount: u64,
    pub accrued_interest_amount: u64,
    pub bump: u8,
}

impl YieldAccount {
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        market: Pubkey,
        asset_mint: Pubkey,
        token_kind: YieldTokenKind,
        recipient: Pubkey,
        bump: u8,
    ) {
        self.owner = owner;
        self.market = market;
        self.asset_mint = asset_mint;
        self.token_kind = token_kind.code();
        self.recipient = recipient;
        self.bump = bump;
    }

    pub fn assert_account(
        &self,
        owner: Pubkey,
        market: Pubkey,
        asset_mint: Pubkey,
        token_kind: YieldTokenKind,
    ) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidYieldAccount);
        require_keys_eq!(self.market, market, ErrorCode::InvalidYieldAccount);
        require_keys_eq!(self.asset_mint, asset_mint, ErrorCode::InvalidYieldAccount);
        require_eq!(
            self.token_kind,
            token_kind.code(),
            ErrorCode::InvalidYieldAccount
        );
        Ok(())
    }

    pub fn accrue(
        &mut self,
        balance: u64,
        swap_fee_growth_index_nad: u128,
        interest_growth_index_nad: u128,
    ) -> Result<()> {
        let swap_fee_amount = accrue_fee_liability(
            balance,
            swap_fee_growth_index_nad,
            self.swap_fee_checkpoint_nad,
        )?;
        let interest_amount = accrue_fee_liability(
            balance,
            interest_growth_index_nad,
            self.interest_checkpoint_nad,
        )?;
        self.accrued_swap_fee_amount = self
            .accrued_swap_fee_amount
            .checked_add(swap_fee_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.accrued_interest_amount = self
            .accrued_interest_amount
            .checked_add(interest_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.swap_fee_checkpoint_nad = swap_fee_growth_index_nad;
        self.interest_checkpoint_nad = interest_growth_index_nad;
        Ok(())
    }

    pub fn claimable_amount(&self) -> Result<u64> {
        self.accrued_swap_fee_amount
            .checked_add(self.accrued_interest_amount)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn clear_claimed(&mut self) {
        self.accrued_swap_fee_amount = 0;
        self.accrued_interest_amount = 0;
    }
}
