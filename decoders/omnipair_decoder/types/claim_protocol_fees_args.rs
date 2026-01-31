

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct ClaimProtocolFeesArgs {
    pub amount0: u64,
    pub amount1: u64,
}
