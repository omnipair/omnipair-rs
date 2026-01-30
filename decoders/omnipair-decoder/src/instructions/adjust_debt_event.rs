
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d9908a974cf749b80")]
pub struct AdjustDebtEvent{
    pub amount0: i64,
    pub amount1: i64,
    pub metadata: EventMetadata,
}
