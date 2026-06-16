// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct MarketConfig {
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub buffer_ratio_bps: u16,
    pub fee_routing_k_nad: u64,
    pub ema_half_life_ms: u64,
    pub directional_ema_half_life_ms: u64,
    pub k_ema_half_life_ms: u64,
    pub max_daily_borrow_bps: u16,
    pub max_daily_withdraw_bps: u16,
    pub spot_ema_divergence_bps: u16,
    pub k_ema_drawdown_bps: u16,
    pub recognized_collateral_cap_bps: u16,
    pub market_health_min_bps: u16,
    pub effective_debt_weight_min_bps: u16,
    pub effective_debt_gamma_nad: u64,
    pub soft_borrow_enabled: bool,
    pub hedged_lp_enabled: bool,
    pub start_time: i64,
}
