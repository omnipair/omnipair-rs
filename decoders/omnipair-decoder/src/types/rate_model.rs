

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct RateModel {
    pub exp_rate: u64,
    pub target_util_start: u64,
    pub target_util_end: u64,
    pub half_life_ms: u64,
    pub min_rate: u64,
    pub max_rate: u64,
    pub initial_rate: u64,
}
