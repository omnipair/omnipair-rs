use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Insurance {
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub base_available: u64,
    pub quote_available: u64,
}
