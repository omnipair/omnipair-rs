
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d83a3d1707b8569eb")]
pub struct ClaimProtocolFeesEvent{
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub futarchy_treasury_amount0: u64,
    pub futarchy_treasury_amount1: u64,
    pub buybacks_vault_amount0: u64,
    pub buybacks_vault_amount1: u64,
    pub team_treasury_amount0: u64,
    pub team_treasury_amount1: u64,
    pub metadata: EventMetadata,
}
