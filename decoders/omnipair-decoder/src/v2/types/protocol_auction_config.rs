// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct ProtocolAuctionConfig {
    pub accepted_mint: solana_pubkey::Pubkey,
    pub recipients: ProtocolAuctionRecipients,
    pub params: ProtocolAuctionParams,
    pub last_settlement_slot: u64,
    pub last_settlement_price_nad: u64,
}
