use anchor_lang::prelude::*;

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
