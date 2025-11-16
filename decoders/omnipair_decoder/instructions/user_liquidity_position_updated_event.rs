
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1dffe3206bd3f6274e")]
pub struct UserLiquidityPositionUpdatedEvent{
    pub token0_amount: u64,
    pub token1_amount: u64,
    pub lp_amount: u64,
    pub token0_mint: solana_pubkey::Pubkey,
    pub token1_mint: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
