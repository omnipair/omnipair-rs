
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct SwapEvent {
    pub reserve0: u64,
    pub reserve1: u64,
    pub is_token0_in: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub amount_in_after_fee: u64,
    pub metadata: EventMetadata,
}
