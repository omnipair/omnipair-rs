// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xb2a9d745aa3b50a0")]
pub struct ProtocolAuctionConfigUpdated {
    pub authority: solana_pubkey::Pubkey,
    pub lane: u8,
    pub accepted_mint: solana_pubkey::Pubkey,
    pub start_multiplier_bps: u16,
    pub floor_multiplier_bps: u16,
    pub duration_slots: u64,
    pub max_reference_age_slots: u64,
    pub signer: solana_pubkey::Pubkey,
}
