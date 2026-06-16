// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct FeeLedger {
    pub fee_growth_index_nad: u128,
    pub hedged_fee_growth_index_nad: u128,
    pub fee_vault_balance: u64,
    pub fee_liability: u64,
    pub hedged_fee_liability: u64,
    pub unallocated_fee_liability: u64,
    pub unallocated_hedged_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub operator_fee_liability: u64,
}
