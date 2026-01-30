
use super::*;

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct PairCreatedEvent {
    pub token0: solana_pubkey::Pubkey,
    pub token1: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub rate_model: solana_pubkey::Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: EventMetadata,
}
