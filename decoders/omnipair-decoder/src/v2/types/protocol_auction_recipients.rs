// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct ProtocolAuctionRecipients {
    pub treasury: solana_pubkey::Pubkey,
    pub staking_vault: solana_pubkey::Pubkey,
    pub treasury_bps: u16,
    pub staking_vault_bps: u16,
}
