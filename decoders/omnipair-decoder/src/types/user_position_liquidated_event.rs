
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UserPositionLiquidatedEvent {
    pub position: solana_pubkey::Pubkey,
    pub liquidator: solana_pubkey::Pubkey,
    pub collateral0_liquidated: u64,
    pub collateral1_liquidated: u64,
    pub debt0_liquidated: u64,
    pub debt1_liquidated: u64,
    pub collateral_price: u64,
    pub shortfall: u128,
    pub liquidation_bonus_applied: u64,
    pub k0: u128,
    pub k1: u128,
    pub metadata: EventMetadata,
}
