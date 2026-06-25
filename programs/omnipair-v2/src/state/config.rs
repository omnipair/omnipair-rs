use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MAX_HALF_LIFE_MS, MAX_MANAGER_FEE_BPS, MIN_HALF_LIFE_MS},
    errors::ErrorCode,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
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

    fn valid_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 30,
            manager_fee_bps: 0,
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
    fn market_config_caps_manager_fee_at_five_percent() {
        let mut config = valid_config();
        config.manager_fee_bps = MAX_MANAGER_FEE_BPS;
        config.validate().unwrap();

        config.manager_fee_bps = MAX_MANAGER_FEE_BPS + 1;
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

}
