
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d9fdec62ada5aac4b")]
pub struct UserPositionTransferredEvent{
    pub from_position: solana_pubkey::Pubkey,
    pub to_position: solana_pubkey::Pubkey,
    pub from_owner: solana_pubkey::Pubkey,
    pub to_owner: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
