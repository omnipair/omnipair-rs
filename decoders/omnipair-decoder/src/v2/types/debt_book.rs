// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct DebtBook {
    pub fixed_base_debt_shares: u128,
    pub fixed_quote_debt_shares: u128,
    pub soft_base_debt_shares: u128,
    pub soft_quote_debt_shares: u128,
    pub base_borrow_index_nad: u128,
    pub quote_borrow_index_nad: u128,
}
