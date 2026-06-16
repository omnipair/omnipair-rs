// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct WithdrawCollateralArgs {
    pub market_asset: MarketAsset,
    pub withdraw_amount: u64,
    pub min_asset_amount_out: u64,
}
