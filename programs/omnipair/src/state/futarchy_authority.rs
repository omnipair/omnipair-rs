use anchor_lang::prelude::*;
#[allow(unused_imports)]
use crate::constants::*;

#[account]
pub struct FutarchyAuthority {
    pub authority: Pubkey,
    pub last_config_nonce: u64,
    pub futarchy_treasury: Pubkey,
    pub futarchy_treasury_percentage_bps: u16,
    pub buybacks_vault: Pubkey,
    pub buybacks_vault_percentage_bps: u16,
    pub team_treasury: Pubkey,
    pub team_treasury_percentage_bps: u16,
    pub bump: u8,
}

impl FutarchyAuthority {
    pub fn initialize(
        authority: Pubkey,
        last_config_nonce: u64,
        futarchy_treasury: Pubkey,
        futarchy_treasury_percentage_bps: u16,
        buybacks_vault: Pubkey,
        buybacks_vault_percentage_bps: u16,
        team_treasury: Pubkey,
        team_treasury_percentage_bps: u16,
        bump: u8,
    ) -> Self {
        Self {
            authority,
            last_config_nonce,
            futarchy_treasury,
            futarchy_treasury_percentage_bps,
            buybacks_vault,
            buybacks_vault_percentage_bps,
            team_treasury,
            team_treasury_percentage_bps,
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