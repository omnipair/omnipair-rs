use anchor_lang::prelude::*;

#[account]
pub struct FutarchyAuthority {
    pub authority: Pubkey,
    pub bump: u8,
}

impl FutarchyAuthority {
    pub fn initialize(authority: Pubkey, bump: u8) -> Self {
        Self {
            authority,
            bump,
        }
    }
}