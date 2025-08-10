use anchor_lang::prelude::*;

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