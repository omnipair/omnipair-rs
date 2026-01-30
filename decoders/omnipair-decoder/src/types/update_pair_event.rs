
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UpdatePairEvent {
    pub price0_ema: u64,
    pub price1_ema: u64,
    pub rate0: u64,
    pub rate1: u64,
    pub accrued_interest0: u128,
    pub accrued_interest1: u128,
    pub protocol_revenue_reserve0: u64,
    pub protocol_revenue_reserve1: u64,
    pub reserve0_after_interest: u64,
    pub reserve1_after_interest: u64,
    pub metadata: EventMetadata,
}
