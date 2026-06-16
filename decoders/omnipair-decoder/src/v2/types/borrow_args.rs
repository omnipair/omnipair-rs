// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct BorrowArgs {
    pub borrow_asset: MarketAsset,
    pub borrow_amount: u64,
    pub min_debt_amount_out: u64,
    pub min_health_bps: u64,
}
