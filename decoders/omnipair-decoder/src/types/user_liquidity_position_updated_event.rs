
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UserLiquidityPositionUpdatedEvent {
    pub token0_amount: u64,
    pub token1_amount: u64,
    pub lp_amount: u64,
    pub cash_reserve0: u64,
    pub cash_reserve1: u64,
    pub token0_mint: solana_pubkey::Pubkey,
    pub token1_mint: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub metadata: EventMetadata,
}
