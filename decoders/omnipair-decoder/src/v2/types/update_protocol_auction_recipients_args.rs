// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct UpdateProtocolAuctionRecipientsArgs {
    pub lane: ProtocolAuctionLane,
    pub treasury: Option<solana_pubkey::Pubkey>,
    pub staking_vault: Option<solana_pubkey::Pubkey>,
    pub treasury_bps: Option<u16>,
    pub staking_vault_bps: Option<u16>,
}
