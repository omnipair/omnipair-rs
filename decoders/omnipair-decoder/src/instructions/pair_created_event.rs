
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xe445a52e51cb9a1d760032c437ff792b")]
pub struct PairCreatedEvent{
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub rate_model: solana_pubkey::Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub target_util_start_bps: u64,
    pub target_util_end_bps: u64,
    pub rate_half_life_ms: u64,
    pub min_rate_bps: u64,
    pub max_rate_bps: u64,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: EventMetadata,
}
