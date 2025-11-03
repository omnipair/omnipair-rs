

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct RateModel {
    pub exp_rate: u64,
    pub target_util_start: u64,
    pub target_util_end: u64,
}
