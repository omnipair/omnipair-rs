
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d53a8c558592a3a66")]
pub struct UserPositionUpdatedEvent{
    pub position: solana_pubkey::Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub debt0_shares: u128,
    pub debt1_shares: u128,
    pub collateral0_applied_min_cf_bps: u16,
    pub collateral1_applied_min_cf_bps: u16,
    pub metadata: EventMetadata,
}
