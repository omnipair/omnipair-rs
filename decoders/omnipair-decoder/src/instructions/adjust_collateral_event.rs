
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d63f6437e2cfcc121")]
pub struct AdjustCollateralEvent{
    pub amount0: i64,
    pub amount1: i64,
    pub metadata: EventMetadata,
}
