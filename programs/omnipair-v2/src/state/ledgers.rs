use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ReserveLedger {
    pub live_reserve: u64,
    pub cash_reserve: u64,
    pub reserved_liability: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ClaimLedger {
    pub protected_claim_supply: u64,
    pub hedged_claim_supply: u64,
    pub staked_claim_supply: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct BufferBook {
    pub buffer_shares: u64,
    pub staked_buffer_shares: u64,
    pub required_buffer: u64,
    pub buffer_ratio_bps: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct FeeLedger {
    pub fee_growth_index_nad: u128,
    pub hedged_fee_growth_index_nad: u128,
    pub fee_vault_balance: u64,
    pub fee_liability: u64,
    pub hedged_fee_liability: u64,
    pub unallocated_fee_liability: u64,
    pub unallocated_hedged_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub operator_fee_liability: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MarketFeeClaimKind {
    Operator,
    Protocol,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DailyLimitBook {
    pub borrowed_bucket: u64,
    pub withdrawn_bucket: u64,
    pub last_decay_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct InsuranceReserve {
    pub vault0: Pubkey,
    pub vault1: Pubkey,
    pub available0: u64,
    pub available1: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RecognitionLedger {
    pub debt_bearing_collateral0_for_debt1: u64,
    pub debt_bearing_collateral1_for_debt0: u64,
    pub last_recognition_slot: u64,
}

impl MarketFeeClaimKind {
    pub fn event_code(self) -> u8 {
        match self {
            Self::Operator => 0,
            Self::Protocol => 1,
        }
    }
}

impl FeeLedger {
    pub fn total_liability(&self) -> Result<u64> {
        self.fee_liability
            .checked_add(self.hedged_fee_liability)
            .and_then(|value| value.checked_add(self.unallocated_hedged_fee_liability))
            .and_then(|value| value.checked_add(self.protocol_fee_liability))
            .and_then(|value| value.checked_add(self.operator_fee_liability))
            .and_then(|value| value.checked_add(self.unallocated_fee_liability))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn assert_backed(&self) -> Result<()> {
        require_gte!(
            self.fee_vault_balance,
            self.total_liability()?,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(())
    }

    pub fn market_fee_liability(&self, claim_kind: MarketFeeClaimKind) -> u64 {
        match claim_kind {
            MarketFeeClaimKind::Operator => self.operator_fee_liability,
            MarketFeeClaimKind::Protocol => self.protocol_fee_liability,
        }
    }

    pub fn claim_market_fee_liability(&mut self, claim_kind: MarketFeeClaimKind) -> Result<u64> {
        let fee_amount = self.market_fee_liability(claim_kind);
        require!(fee_amount > 0, ErrorCode::AmountZero);
        match claim_kind {
            MarketFeeClaimKind::Operator => self.operator_fee_liability = 0,
            MarketFeeClaimKind::Protocol => self.protocol_fee_liability = 0,
        }
        Ok(fee_amount)
    }
}
