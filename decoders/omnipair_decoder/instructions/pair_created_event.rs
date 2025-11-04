
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d760032c437ff792b")]
pub struct PairCreatedEvent{
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
