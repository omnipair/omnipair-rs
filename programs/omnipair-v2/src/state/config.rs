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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::NAD;

    fn valid_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 30,
            operator_fee_bps: 1_000,
            protocol_fee_bps: 0,
            buffer_ratio_bps: 2_000,
            fee_routing_k_nad: NAD,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: 2_000,
            max_daily_withdraw_bps: 2_000,
            spot_ema_divergence_bps: 1_000,
            k_ema_drawdown_bps: 1_000,
            recognized_collateral_cap_bps: 15_000,
            market_health_min_bps: 11_000,
            effective_debt_weight_min_bps: 10_000,
            effective_debt_gamma_nad: NAD,
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
    fn market_config_rejects_inert_fee_routing() {
        let mut config = valid_config();
        config.fee_routing_k_nad = 0;

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
}
