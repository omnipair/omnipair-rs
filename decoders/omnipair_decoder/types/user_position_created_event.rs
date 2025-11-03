
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UserPositionCreatedEvent {
    pub position: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
