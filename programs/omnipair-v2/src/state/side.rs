use anchor_lang::prelude::*;

use super::{BufferLedger, ClaimTokenLedger, DailyLimitBook, FeeLedger, ReserveLedger};
use crate::{errors::ErrorCode, utils::market_math::required_buffer_for_claims};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketAsset {
    Base,
    Quote,
}

impl MarketAsset {
    pub fn opposite(self) -> Self {
        match self {
            Self::Base => Self::Quote,
            Self::Quote => Self::Base,
        }
    }

    pub fn is_base(self) -> bool {
        matches!(self, Self::Base)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSide {
    pub asset_mint: Pubkey,
    pub asset_decimals: u8,
    pub claim_token_mint: Pubkey,
    pub hedge_token_mint: Pubkey,
    pub hedge_vault: Pubkey,
    pub reserve_vault: Pubkey,
    pub collateral_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub stake_vault: Pubkey,
    pub reserve_ledger: ReserveLedger,
    pub claim_token_ledger: ClaimTokenLedger,
    pub buffer_ledger: BufferLedger,
    pub fee_ledger: FeeLedger,
    pub daily_limit_book: DailyLimitBook,
}

impl MarketSide {
    pub fn claim_floor(&self) -> Result<u64> {
        self.claim_token_ledger
            .protected_claim_token_supply
            .checked_add(self.buffer_ledger.required_buffer)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn free_buffer(&self) -> Result<u64> {
        self.reserve_ledger
            .live_reserve
            .checked_sub(self.claim_token_ledger.protected_claim_token_supply)
            .ok_or(ErrorCode::InsufficientMarketClaimCoverage.into())
    }

    pub fn assert_claim_coverage(&self) -> Result<()> {
        require_gte!(
            self.reserve_ledger.live_reserve,
            self.claim_floor()?,
            ErrorCode::InsufficientMarketClaimCoverage
        );
        Ok(())
    }

    pub fn required_buffer_for_ratio(&self, buffer_ratio_bps: u16) -> Result<u64> {
        required_buffer_for_claims(
            self.claim_token_ledger.protected_claim_token_supply,
            buffer_ratio_bps,
        )
    }

    pub fn assert_buffer_floor_for_ratio(&self, buffer_ratio_bps: u16) -> Result<u64> {
        let required_buffer = self.required_buffer_for_ratio(buffer_ratio_bps)?;
        require_gte!(
            self.buffer_ledger.buffer_share_supply,
            required_buffer,
            ErrorCode::InsufficientBufferShares
        );
        require_gte!(
            self.reserve_ledger.live_reserve,
            self.claim_token_ledger
                .protected_claim_token_supply
                .checked_add(required_buffer)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            ErrorCode::InsufficientMarketClaimCoverage
        );
        Ok(required_buffer)
    }

    pub fn apply_buffer_ratio(&mut self, buffer_ratio_bps: u16, required_buffer: u64) {
        self.buffer_ledger.buffer_ratio_bps = buffer_ratio_bps;
        self.buffer_ledger.required_buffer = required_buffer;
    }
}
