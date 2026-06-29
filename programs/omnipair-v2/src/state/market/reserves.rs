use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Reserves {
    pub live_reserve: u64,
    pub cash_reserve: u64,
    pub reserved_liability: u64,
}
