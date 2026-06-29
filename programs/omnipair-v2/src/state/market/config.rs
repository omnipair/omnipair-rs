use anchor_lang::prelude::*;

use crate::{
    constants::{
        BPS_DENOMINATOR, LIQUIDATION_MAX_INCENTIVE_BPS, MAX_HALF_LIFE_MS, MAX_MANAGER_FEE_BPS,
        MIN_HALF_LIFE_MS,
    },
    errors::ErrorCode,
};

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq, Eq,
)]
pub struct MarketConfig {
    pub swap_fee_bps: u16,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub target_hlp_leverage_bps: u16,
    pub settlement_divergence_bps: u16,
    pub emergency_exit_haircut_bps: u16,
    pub ema_half_life_ms: u64,
    pub directional_ema_half_life_ms: u64,
    pub k_ema_half_life_ms: u64,
    pub max_daily_borrow_bps: u16,
    pub max_daily_withdraw_bps: u16,
    pub spot_ema_divergence_bps: u16,
    pub k_ema_drawdown_bps: u16,
    pub recognized_collateral_cap_bps: u16,
    pub market_health_min_bps: u16,
    pub liquidation_auction_duration_slots: u64,
    pub liquidation_auction_start_incentive_bps: u16,
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
            MAX_MANAGER_FEE_BPS,
            self.manager_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require!(self.protocol_fee_bps == 0, ErrorCode::InvalidMarketConfig);
        require_eq!(
            self.target_hlp_leverage_bps,
            BPS_DENOMINATOR
                .checked_mul(2)
                .ok_or(ErrorCode::InvalidMarketConfig)?,
            ErrorCode::InvalidMarketConfig
        );
        require!(
            self.max_daily_borrow_bps <= BPS_DENOMINATOR
                && self.max_daily_withdraw_bps <= BPS_DENOMINATOR
                && self.spot_ema_divergence_bps <= BPS_DENOMINATOR
                && self.k_ema_drawdown_bps <= BPS_DENOMINATOR
                && self.settlement_divergence_bps <= BPS_DENOMINATOR
                && self.emergency_exit_haircut_bps <= BPS_DENOMINATOR,
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
                && self.recognized_collateral_cap_bps >= self.market_health_min_bps,
            ErrorCode::InvalidMarketConfig
        );
        require!(
            self.liquidation_auction_duration_slots > 0
                && self.liquidation_auction_start_incentive_bps <= LIQUIDATION_MAX_INCENTIVE_BPS,
            ErrorCode::InvalidMarketConfig
        );
        Ok(())
    }
}

fn half_life_in_bounds(half_life_ms: u64) -> bool {
    (MIN_HALF_LIFE_MS..=MAX_HALF_LIFE_MS).contains(&half_life_ms)
}

#[cfg(test)]
mod tests {
    include!("../../tests/state/config.rs");
}
