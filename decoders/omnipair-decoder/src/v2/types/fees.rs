// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct Fees {
    pub swap_fee_growth_index_nad: u128,
    pub interest_growth_index_nad: u128,
    pub swap_fee_vault_balance: u64,
    pub interest_vault_balance: u64,
    pub swap_fee_liability: u64,
    pub interest_liability: u64,
    pub unallocated_swap_fee_liability: u64,
    pub unallocated_interest_liability: u64,
    pub protocol_fee_liability: u64,
    pub buyback_fee_liability: u64,
    pub operator_fee_liability: u64,
}
