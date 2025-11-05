

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct Pair {
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub rate_model: solana_pubkey::Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub reserve0: u64,
    pub reserve1: u64,
    pub protocol_revenue_reserve0: u64,
    pub protocol_revenue_reserve1: u64,
    pub last_price0_ema: u64,
    pub last_price1_ema: u64,
    pub last_update: i64,
    pub last_rate0: u64,
    pub last_rate1: u64,
    pub total_debt0: u64,
    pub total_debt1: u64,
    pub total_debt0_shares: u64,
    pub total_debt1_shares: u64,
    pub total_supply: u64,
    pub total_collateral0: u64,
    pub total_collateral1: u64,
    pub pair_nonce: [u8; 16],
    pub bump: u8,
}
