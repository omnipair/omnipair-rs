// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct SettleProtocolAuctionArgs {
    pub lane: ProtocolAuctionLane,
    pub side: MarketAsset,
    pub sold_amount: u64,
    pub max_payment_amount: u64,
}
