
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UserPositionTransferredEvent {
    pub from_position: solana_pubkey::Pubkey,
    pub to_position: solana_pubkey::Pubkey,
    pub from_owner: solana_pubkey::Pubkey,
    pub to_owner: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
