// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct UnstakeArgs {
    pub market_asset: MarketAsset,
    pub claim_amount: u64,
    pub buffer_share_amount: u64,
}
