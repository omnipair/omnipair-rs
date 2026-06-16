// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct MarketUpdated {
    pub market: solana_pubkey::Pubkey,
    pub reduce_only: bool,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub metadata: MarketEventMetadata,
}
