// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct HlpRebalanced {
    pub market: solana_pubkey::Pubkey,
    pub target_side: u8,
    pub ideal_delta: i128,
    pub executed_delta: i128,
    pub pending_rebalance: i128,
    pub nav_nad: u128,
    pub metadata: MarketEventMetadata,
}
