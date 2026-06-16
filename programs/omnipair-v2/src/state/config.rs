use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MAX_HALF_LIFE_MS, MIN_HALF_LIFE_MS},
    errors::ErrorCode,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
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

impl MarketConfig {
    pub fn validate(&self) -> Result<()> {
        require_gte!(
            BPS_DENOMINATOR,
            self.swap_fee_bps,
            ErrorCode::InvalidSwapFeeBps
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.protocol_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps
                .checked_add(self.protocol_fee_bps)
                .ok_or(ErrorCode::InvalidMarketConfig)?,
            ErrorCode::InvalidMarketConfig
        );
        require!(
            self.buffer_ratio_bps > 0 && self.buffer_ratio_bps < BPS_DENOMINATOR,
            ErrorCode::InvalidMarketBufferRatio
        );
        require!(self.fee_routing_k_nad > 0, ErrorCode::InvalidMarketConfig);
        require!(
            self.max_daily_borrow_bps <= BPS_DENOMINATOR
                && self.max_daily_withdraw_bps <= BPS_DENOMINATOR
                && self.spot_ema_divergence_bps <= BPS_DENOMINATOR
                && self.k_ema_drawdown_bps <= BPS_DENOMINATOR,
            ErrorCode::InvalidMarketConfig
        );
        require!(
            half_life_in_bounds(self.ema_half_life_ms)
                && half_life_in_bounds(self.directional_ema_half_life_ms)
                && half_life_in_bounds(self.k_ema_half_life_ms),
            ErrorCode::InvalidMarketConfig
        );
        require!(
            self.recognized_collateral_cap_bps >= BPS_DENOMINATOR
                && self.market_health_min_bps >= BPS_DENOMINATOR
                && self.recognized_collateral_cap_bps >= self.market_health_min_bps
                && self.effective_debt_weight_min_bps <= BPS_DENOMINATOR,
            ErrorCode::InvalidMarketConfig
        );
        require!(!self.soft_borrow_enabled, ErrorCode::InvalidMarketConfig);
        Ok(())
    }
}

fn half_life_in_bounds(half_life_ms: u64) -> bool {
    (MIN_HALF_LIFE_MS..=MAX_HALF_LIFE_MS).contains(&half_life_ms)
}
