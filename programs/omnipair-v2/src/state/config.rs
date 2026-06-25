use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MAX_HALF_LIFE_MS, MIN_HALF_LIFE_MS},
    errors::ErrorCode,
    math::interest::InterestRateParams,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketConfig {
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
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
    // Kinked borrow-interest curve (APR in bps). See `math::interest`.
    pub interest_base_rate_bps: u16,
    pub interest_slope1_bps: u16,
    pub interest_optimal_utilization_bps: u16,
    pub interest_slope2_bps: u16,
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
        require!(
            self.operator_fee_bps == 0 && self.protocol_fee_bps == 0,
            ErrorCode::InvalidMarketConfig
        );
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
        // The kink must be a strict interior point so the curve is well-defined
        // on both sides; rate magnitudes are bounded by their u16 width.
        require!(
            self.interest_optimal_utilization_bps > 0
                && self.interest_optimal_utilization_bps < BPS_DENOMINATOR,
            ErrorCode::InvalidMarketConfig
        );
        require!(!self.soft_borrow_enabled, ErrorCode::InvalidMarketConfig);
        Ok(())
    }

    pub fn interest_rate_params(&self) -> InterestRateParams {
        InterestRateParams {
            base_rate_bps: self.interest_base_rate_bps as u64,
            slope1_bps: self.interest_slope1_bps as u64,
            optimal_utilization_bps: self.interest_optimal_utilization_bps as u64,
            slope2_bps: self.interest_slope2_bps as u64,
        }
    }
}

fn half_life_in_bounds(half_life_ms: u64) -> bool {
    (MIN_HALF_LIFE_MS..=MAX_HALF_LIFE_MS).contains(&half_life_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 30,
            operator_fee_bps: 0,
            protocol_fee_bps: 0,
            target_hlp_leverage_bps: 20_000,
            settlement_divergence_bps: 500,
            emergency_exit_haircut_bps: 250,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: 2_000,
            max_daily_withdraw_bps: 2_000,
            spot_ema_divergence_bps: 1_000,
            k_ema_drawdown_bps: 1_000,
            recognized_collateral_cap_bps: 15_000,
            market_health_min_bps: 11_000,
            interest_base_rate_bps: 0,
            interest_slope1_bps: 1_000,
            interest_optimal_utilization_bps: 8_000,
            interest_slope2_bps: 30_000,
            soft_borrow_enabled: false,
            hedged_lp_enabled: true,
            start_time: 0,
        }
    }

    #[test]
    fn market_config_rejects_soft_borrow_until_implemented() {
        let mut config = valid_config();
        config.soft_borrow_enabled = true;

        let err = config.validate().unwrap_err();

        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_recognition_cap_below_health_floor() {
        let mut config = valid_config();
        config.recognized_collateral_cap_bps = 10_000;
        config.market_health_min_bps = 11_000;

        let err = config.validate().unwrap_err();

        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_inert_ema_half_lives() {
        let mut config = valid_config();
        config.ema_half_life_ms = 0;
        assert_eq!(
            config.validate().unwrap_err(),
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );

        let mut config = valid_config();
        config.directional_ema_half_life_ms = MIN_HALF_LIFE_MS - 1;
        assert_eq!(
            config.validate().unwrap_err(),
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );

        let mut config = valid_config();
        config.k_ema_half_life_ms = MAX_HALF_LIFE_MS + 1;
        assert_eq!(
            config.validate().unwrap_err(),
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_invalid_hlp_leverage() {
        let mut config = valid_config();
        config.target_hlp_leverage_bps = 19_999;

        let err = config.validate().unwrap_err();

        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_invalid_k_drawdown_limit() {
        let mut config = valid_config();
        config.k_ema_drawdown_bps = BPS_DENOMINATOR + 1;

        let err = config.validate().unwrap_err();

        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_degenerate_interest_kink() {
        // The kink must be a strict interior point of (0, 100%).
        let mut config = valid_config();
        config.interest_optimal_utilization_bps = 0;
        assert_eq!(
            config.validate().unwrap_err(),
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );

        let mut config = valid_config();
        config.interest_optimal_utilization_bps = BPS_DENOMINATOR;
        assert_eq!(
            config.validate().unwrap_err(),
            anchor_lang::prelude::error!(ErrorCode::InvalidMarketConfig)
        );
    }
}
