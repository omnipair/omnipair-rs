use anchor_lang::prelude::*;
#[allow(unused_imports)]
use crate::constants::*;

#[account]
pub struct FutarchyAuthority {
    pub authority: Pubkey,
    pub last_config_nonce: u64,
    pub bump: u8,
}

impl FutarchyAuthority {
    pub fn initialize(authority: Pubkey, last_config_nonce: u64, bump: u8) -> Self {
        Self {
            authority,
            last_config_nonce,
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