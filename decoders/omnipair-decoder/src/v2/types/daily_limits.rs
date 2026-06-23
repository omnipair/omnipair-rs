// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct DailyLimits {
    pub borrowed_bucket: u64,
    pub withdrawn_bucket: u64,
    pub last_decay_slot: u64,
}
