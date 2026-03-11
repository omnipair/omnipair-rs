
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d2231eff2e42d1461")]
pub struct FlashloanEvent{
    pub amount0: u64,
    pub amount1: u64,
    pub fee0: u64,
    pub fee1: u64,
    pub receiver: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
