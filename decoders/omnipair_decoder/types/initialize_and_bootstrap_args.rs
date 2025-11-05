

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct InitializeAndBootstrapArgs {
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub pair_nonce: [u8; 16],
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,
    pub lp_name: String,
    pub lp_symbol: String,
    pub lp_uri: String,
}
