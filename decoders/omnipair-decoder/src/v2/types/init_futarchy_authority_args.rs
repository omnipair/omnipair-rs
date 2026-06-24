// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct InitFutarchyAuthorityArgs {
    pub authority: solana_pubkey::Pubkey,
    pub swap_bps: u16,
    pub interest_bps: u16,
    pub futarchy_treasury: solana_pubkey::Pubkey,
    pub futarchy_treasury_bps: u16,
    pub buybacks_vault: solana_pubkey::Pubkey,
    pub buybacks_vault_bps: u16,
    pub team_treasury: solana_pubkey::Pubkey,
    pub team_treasury_bps: u16,
    pub staking_vault: solana_pubkey::Pubkey,
    pub fee_auction_accepted_mint: solana_pubkey::Pubkey,
    pub buyback_auction_accepted_mint: solana_pubkey::Pubkey,
}
