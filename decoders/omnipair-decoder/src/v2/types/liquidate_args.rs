// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct LiquidateArgs {
    pub debt_asset: MarketAsset,
    pub repay_amount: u64,
    pub min_collateral_out: u64,
    pub max_insurance_draw: u64,
    pub max_socialized_loss: u64,
}
