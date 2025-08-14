use anchor_lang::prelude::*;

#[account]
pub struct PairConfig {
    pub futarchy_fee_bps: u16,
    pub founder_fee_bps: u16,
    pub nonce: u64,
    pub bump: u8,
}

impl PairConfig {
    pub fn initialize(
        futarchy_fee_bps: u16, 
        founder_fee_bps: u16, 
        nonce: u64,
        bump: u8,
    ) -> Self {
        Self {
            futarchy_fee_bps,
            founder_fee_bps,
            nonce,
            bump,
        }
    }

    pub fn update_if_some<T>(field: &mut T, new_value: Option<T>) {
        if let Some(value) = new_value {
            *field = value;
        }
    }
}