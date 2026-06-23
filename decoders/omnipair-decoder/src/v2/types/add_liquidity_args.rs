// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct AddLiquidityArgs {
    pub base_deposit_amount: u64,
    pub quote_deposit_amount: u64,
    pub min_base_ylp_amount: u64,
    pub min_quote_ylp_amount: u64,
}
