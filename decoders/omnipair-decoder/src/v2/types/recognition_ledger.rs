// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct RecognitionLedger {
    pub debt_bearing_base_collateral_for_quote_debt: u64,
    pub debt_bearing_quote_collateral_for_base_debt: u64,
    pub last_recognition_slot: u64,
}
