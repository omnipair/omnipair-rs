
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d40c6cde8260871e2")]
pub struct SwapEvent{
    pub reserve0: u64,
    pub reserve1: u64,
    pub is_token0_in: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub amount_in_after_fee: u64,
    pub metadata: EventMetadata,
}
