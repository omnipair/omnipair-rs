use anchor_lang::prelude::*;
#[allow(unused_imports)]
use crate::constants::*;

#[account]
pub struct FutarchyAuthority {
    pub authority: Pubkey,
    pub last_config_nonce: u64,
    pub recipient1: Pubkey,
    pub recipient1_percentage_bps: u16,
    pub recipient2: Pubkey,
    pub recipient2_percentage_bps: u16,
    pub recipient3: Pubkey,
    pub recipient3_percentage_bps: u16,
    pub bump: u8,
}

impl FutarchyAuthority {
    pub fn initialize(
        authority: Pubkey,
        last_config_nonce: u64,
        recipient1: Pubkey,
        recipient1_percentage_bps: u16,
        recipient2: Pubkey,
        recipient2_percentage_bps: u16,
        recipient3: Pubkey,
        recipient3_percentage_bps: u16,
        bump: u8,
    ) -> Self {
        Self {
            authority,
            last_config_nonce,
            recipient1,
            recipient1_percentage_bps,
            recipient2,
            recipient2_percentage_bps,
            recipient3,
            recipient3_percentage_bps,
            bump,
        }
    }
}

#[macro_export]
macro_rules! generate_futarchy_authority_seeds {
    ($futarchy_authority:expr) => {{
        &[
            FUTARCHY_AUTHORITY_SEED_PREFIX,
            &[$futarchy_authority.bump],
        ]
    }};
}