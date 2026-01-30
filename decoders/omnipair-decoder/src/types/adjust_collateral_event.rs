
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct AdjustCollateralEvent {
    pub amount0: i64,
    pub amount1: i64,
    pub metadata: EventMetadata,
}
