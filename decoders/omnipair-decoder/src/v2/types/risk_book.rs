// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct RiskBook {
    pub base_price_ema_nad: u64,
    pub quote_price_ema_nad: u64,
    pub directional_base_price_ema_nad: u64,
    pub directional_quote_price_ema_nad: u64,
    pub cached_spot_base_price_nad: u64,
    pub cached_spot_quote_price_nad: u64,
    pub cached_k_nad: u128,
    pub cached_liquidity_nad: u128,
    pub cached_base_liquidity_nad: u128,
    pub cached_quote_liquidity_nad: u128,
    pub k_ema: u128,
    pub liquidity_ema: u128,
    pub base_liquidity_ema: u128,
    pub quote_liquidity_ema: u128,
    pub last_snapshot_slot: u64,
}
