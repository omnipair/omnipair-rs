// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct UpdateProtocolAuctionConfigArgs {
    pub lane: ProtocolAuctionLane,
    pub accepted_mint: Option<solana_pubkey::Pubkey>,
    pub params: Option<ProtocolAuctionParams>,
}
