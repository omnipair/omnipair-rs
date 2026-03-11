

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UserPosition {
    pub owner: solana_pubkey::Pubkey,
    pub pair: solana_pubkey::Pubkey,
    pub collateral0_liquidation_cf_bps: u16,
    pub collateral1_liquidation_cf_bps: u16,
    pub collateral0: u64,
    pub collateral1: u64,
    pub debt0_shares: u128,
    pub debt1_shares: u128,
    pub bump: u8,
}
