// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xaff7a0b68c80d3e2")]
pub struct FutarchyAuthority {
    pub version: u8,
    pub authority: solana_pubkey::Pubkey,
    pub recipients: RevenueRecipients,
    pub revenue_share: RevenueShare,
    pub revenue_distribution: RevenueDistribution,
    pub protocol_auction_split: ProtocolAuctionSplit,
    pub fee_auction: ProtocolAuctionConfig,
    pub buyback_auction: ProtocolAuctionConfig,
    pub global_reduce_only: bool,
    pub bump: u8,
}
