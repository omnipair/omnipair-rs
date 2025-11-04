
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct PairCreatedEvent {
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
