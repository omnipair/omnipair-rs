use anchor_lang::prelude::*;

#[account]
pub struct PairConfig {
    pub rate_model: Pubkey,
    pub swap_fee_bps: u16,
    pub futarchy_fee_bps: u16,
    pub founder_fee_bps: u16,
    pub nonce: u64,
}

impl PairConfig {
    pub fn initialize(
        rate_model: Pubkey, 
        swap_fee_bps: u16, 
        futarchy_fee_bps: u16, 
        founder_fee_bps: u16, 
        nonce: u64,
    ) -> Self {
        Self {
            rate_model,
            swap_fee_bps,
            futarchy_fee_bps,
            founder_fee_bps,
            nonce,
        }
    }

    pub fn update_if_some<T>(field: &mut T, new_value: Option<T>) {
        if let Some(value) = new_value {
            *field = value;
        }
    }
}