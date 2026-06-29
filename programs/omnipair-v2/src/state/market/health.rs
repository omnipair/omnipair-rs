use anchor_lang::prelude::*;

use super::{Market, MarketAsset, MarketHealth, Risk};
use crate::{
    constants::{BPS_DENOMINATOR, LIQUIDATION_INCENTIVE_BPS, LIQUIDATION_PENALTY_BPS},
    errors::ErrorCode,
    math::*,
    shared::math::ceil_div,
    state::MarginPosition,
};

impl Market {
    pub fn refresh_market_health(&mut self) -> Result<()> {
        self.refresh_risk()
    }

    pub fn recompute_market_health_from_risk(&mut self) -> Result<()> {
        let effective_base_debt_nad = self.effective_base_debt_nad()?;
        let effective_quote_debt_nad = self.effective_quote_debt_nad()?;
        let quote_collateral_value_for_base_debt_nad = self
            .quote_collateral_value_for_base_debt_nad(
                self.debt.recognized_quote_collateral_for_base_debt,
            )?;
        let base_collateral_value_for_quote_debt_nad = self
            .base_collateral_value_for_quote_debt_nad(
                self.debt.recognized_base_collateral_for_quote_debt,
            )?;
        let base_debt_health_bps = health_bps(
            quote_collateral_value_for_base_debt_nad,
            effective_base_debt_nad,
        )?;
        let quote_debt_health_bps = health_bps(
            base_collateral_value_for_quote_debt_nad,
            effective_quote_debt_nad,
        )?;
        self.health = MarketHealth {
            recognized_base_collateral_for_quote_debt: self
                .debt
                .recognized_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: self
                .debt
                .recognized_quote_collateral_for_base_debt,
            effective_base_debt_nad,
            effective_quote_debt_nad,
            base_debt_health_bps,
            quote_debt_health_bps,
        };
        Ok(())
    }

    pub fn current_risk(&self) -> Result<Risk> {
        let current_slot = Clock::get()
            .map(|clock| clock.slot)
            .unwrap_or(self.last_update_slot);
        self.risk.refreshed(
            &self.base_side,
            &self.quote_side,
            &self.config,
            current_slot,
        )
    }

    pub fn refresh_risk(&mut self) -> Result<()> {
        self.risk = self.current_risk()?;
        self.last_update_slot = self.risk.last_snapshot_slot;
        self.recompute_market_health_from_risk()
    }

    pub fn enforce_daily_borrow_limit(
        &mut self,
        market_asset: MarketAsset,
        amount: u64,
    ) -> Result<()> {
        self.refresh_risk()?;
        let current_slot = self.risk.last_snapshot_slot;
        let limit = self.daily_limit_for_side(market_asset, self.config.max_daily_borrow_bps)?;
        self.side_mut(market_asset)?
            .daily_limits
            .record_borrow(amount, limit, current_slot)
    }

    pub fn enforce_daily_withdraw_limit(
        &mut self,
        market_asset: MarketAsset,
        amount: u64,
    ) -> Result<()> {
        self.refresh_risk()?;
        let current_slot = self.risk.last_snapshot_slot;
        let limit = self.daily_limit_for_side(market_asset, self.config.max_daily_withdraw_bps)?;
        self.side_mut(market_asset)?
            .daily_limits
            .record_withdraw(amount, limit, current_slot)
    }

    pub fn assert_spot_ema_divergence(&self) -> Result<()> {
        assert_price_divergence(
            market_spot_price_nad(&self.base_side, &self.quote_side)?,
            self.risk.base_price_ema_nad,
            self.config.spot_ema_divergence_bps,
        )?;
        assert_price_divergence(
            market_spot_price_nad(&self.quote_side, &self.base_side)?,
            self.risk.quote_price_ema_nad,
            self.config.spot_ema_divergence_bps,
        )
    }

    pub fn assert_risk_circuit_breakers(&self) -> Result<()> {
        self.assert_spot_ema_divergence()?;
        self.assert_k_ema_drawdown()
    }

    pub fn assert_k_ema_drawdown(&self) -> Result<()> {
        if self.risk.k_ema == 0 {
            return Ok(());
        }
        assert_k_drawdown(
            market_k_nad(&self.base_side, &self.quote_side)?,
            self.risk.k_ema,
            self.config.k_ema_drawdown_bps,
        )
    }

    pub fn effective_base_debt_nad(&self) -> Result<u128> {
        self.effective_debt_nad(MarketAsset::Base)
    }

    pub fn effective_quote_debt_nad(&self) -> Result<u128> {
        self.effective_debt_nad(MarketAsset::Quote)
    }

    pub fn quote_collateral_value_for_base_debt_nad(
        &self,
        quote_collateral_amount: u64,
    ) -> Result<u128> {
        self.collateral_value_nad(MarketAsset::Quote, quote_collateral_amount, &self.risk)
    }

    pub fn base_collateral_value_for_quote_debt_nad(
        &self,
        base_collateral_amount: u64,
    ) -> Result<u128> {
        self.collateral_value_nad(MarketAsset::Base, base_collateral_amount, &self.risk)
    }

    pub fn collateral_amount_for_debt_value(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
    ) -> Result<u64> {
        self.collateral_amount_for_debt_value_with_penalty_bps(
            debt_asset,
            debt_amount,
            LIQUIDATION_PENALTY_BPS,
        )
    }

    pub fn collateral_amount_for_liquidator_debt_value(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
    ) -> Result<u64> {
        self.collateral_amount_for_debt_value_with_penalty_bps(
            debt_asset,
            debt_amount,
            LIQUIDATION_INCENTIVE_BPS,
        )
    }

    pub(crate) fn collateral_amount_for_debt_value_with_penalty_bps(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
        penalty_bps: u16,
    ) -> Result<u64> {
        self.collateral_amount_for_debt_value_with_penalty(
            debt_asset,
            debt_amount,
            penalty_bps,
            &self.current_risk()?,
        )
    }

    pub fn debt_capped_recognized_collateral(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        risk: &Risk,
    ) -> Result<u64> {
        let cap_bps = self.config.recognized_collateral_cap_bps as u128;
        let (fixed_debt, debt_decimals, total_collateral) = match debt_asset {
            MarketAsset::Base => (
                margin_position.fixed_base_debt(&self.debt)?,
                self.base_side.asset_decimals,
                margin_position.quote_collateral,
            ),
            MarketAsset::Quote => (
                margin_position.fixed_quote_debt(&self.debt)?,
                self.quote_side.asset_decimals,
                margin_position.base_collateral,
            ),
        };
        if fixed_debt == 0 || total_collateral == 0 {
            return Ok(0);
        }

        let debt_value_nad = normalize_to_nad(fixed_debt, debt_decimals)?;
        let recognized_value_cap_nad = debt_value_nad
            .checked_mul(cap_bps)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let capped_collateral = self.collateral_amount_for_debt_value_cap_with_risk(
            debt_asset,
            recognized_value_cap_nad,
            risk,
        )?;
        Ok(total_collateral.min(capped_collateral))
    }

    pub fn position_health_bps(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
    ) -> Result<u64> {
        let risk = self.current_risk()?;
        match debt_asset {
            MarketAsset::Base => health_bps(
                self.collateral_value_nad(
                    MarketAsset::Quote,
                    margin_position.recognized_quote_collateral_for_base_debt,
                    &risk,
                )?,
                normalize_to_nad(
                    margin_position.fixed_base_debt(&self.debt)?,
                    self.base_side.asset_decimals,
                )?,
            ),
            MarketAsset::Quote => health_bps(
                self.collateral_value_nad(
                    MarketAsset::Base,
                    margin_position.recognized_base_collateral_for_quote_debt,
                    &risk,
                )?,
                normalize_to_nad(
                    margin_position.fixed_quote_debt(&self.debt)?,
                    self.quote_side.asset_decimals,
                )?,
            ),
        }
    }

    pub fn assert_position_health(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        min_health_bps: u64,
    ) -> Result<()> {
        require_gte!(
            self.position_health_bps(margin_position, debt_asset)?,
            min_health_bps,
            ErrorCode::InsufficientMarketHealth
        );
        Ok(())
    }

    pub fn assert_recognition_cap(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
    ) -> Result<()> {
        let risk = self.current_risk()?;
        let max_recognized =
            self.debt_capped_recognized_collateral(margin_position, debt_asset, &risk)?;
        let recognized = match debt_asset {
            MarketAsset::Base => margin_position.recognized_quote_collateral_for_base_debt,
            MarketAsset::Quote => margin_position.recognized_base_collateral_for_quote_debt,
        };
        require_gte!(
            max_recognized,
            recognized,
            ErrorCode::InsufficientRecognizedCollateral
        );
        Ok(())
    }

    pub fn assert_market_health(&self) -> Result<()> {
        if self.health.effective_base_debt_nad > 0 {
            require_gte!(
                self.health.base_debt_health_bps,
                self.config.market_health_min_bps as u64,
                ErrorCode::InsufficientMarketHealth
            );
        }
        if self.health.effective_quote_debt_nad > 0 {
            require_gte!(
                self.health.quote_debt_health_bps,
                self.config.market_health_min_bps as u64,
                ErrorCode::InsufficientMarketHealth
            );
        }
        Ok(())
    }

    fn effective_debt_nad(&self, debt_asset: MarketAsset) -> Result<u128> {
        let (fixed_debt, debt_side) = match debt_asset {
            MarketAsset::Base => (self.debt.fixed_base_debt()?, &self.base_side),
            MarketAsset::Quote => (self.debt.fixed_quote_debt()?, &self.quote_side),
        };
        normalize_to_nad(fixed_debt, debt_side.asset_decimals)
    }

    pub(crate) fn collateral_value_nad(
        &self,
        collateral_asset: MarketAsset,
        collateral_amount: u64,
        risk: &Risk,
    ) -> Result<u128> {
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match collateral_asset {
                MarketAsset::Base => (
                    &self.base_side,
                    &self.quote_side,
                    risk.base_price_ema_nad,
                    risk.directional_base_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.quote_side,
                    &self.base_side,
                    risk.quote_price_ema_nad,
                    risk.directional_quote_price_ema_nad,
                ),
            };

        collateral_value_from_pessimistic_reserves_nad(
            collateral_side.reserves.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserves.live_reserve,
            debt_side.asset_decimals,
            collateral_amount,
            price_ema_nad,
            directional_price_ema_nad,
        )
    }

    fn collateral_amount_for_debt_value_with_penalty(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
        penalty_bps: u16,
        risk: &Risk,
    ) -> Result<u64> {
        require_gte!(
            LIQUIDATION_PENALTY_BPS,
            LIQUIDATION_INCENTIVE_BPS,
            ErrorCode::InvalidMarketConfig
        );
        let debt_with_penalty = ceil_div(
            (debt_amount as u128)
                .checked_mul((BPS_DENOMINATOR + penalty_bps) as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::MarketMathOverflow)?;
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match debt_asset {
                MarketAsset::Base => (
                    &self.quote_side,
                    &self.base_side,
                    risk.quote_price_ema_nad,
                    risk.directional_quote_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.base_side,
                    &self.quote_side,
                    risk.base_price_ema_nad,
                    risk.directional_base_price_ema_nad,
                ),
            };

        collateral_amount_for_debt_amount_ceil(
            collateral_side.reserves.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserves.live_reserve,
            debt_side.asset_decimals,
            debt_with_penalty,
            price_ema_nad,
            directional_price_ema_nad,
        )
    }

    fn collateral_amount_for_debt_value_cap_with_risk(
        &self,
        debt_asset: MarketAsset,
        debt_value_nad: u128,
        risk: &Risk,
    ) -> Result<u64> {
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match debt_asset {
                MarketAsset::Base => (
                    &self.quote_side,
                    &self.base_side,
                    risk.quote_price_ema_nad,
                    risk.directional_quote_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.base_side,
                    &self.quote_side,
                    risk.base_price_ema_nad,
                    risk.directional_base_price_ema_nad,
                ),
            };

        collateral_amount_for_debt_value_floor(
            collateral_side.reserves.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserves.live_reserve,
            debt_side.asset_decimals,
            debt_value_nad,
            price_ema_nad,
            directional_price_ema_nad,
        )
    }

    fn daily_limit_for_side(&self, market_asset: MarketAsset, limit_bps: u16) -> Result<u64> {
        let (liquidity_ema, asset_decimals) = match market_asset {
            MarketAsset::Base => (self.risk.base_liquidity_ema, self.base_side.asset_decimals),
            MarketAsset::Quote => (
                self.risk.quote_liquidity_ema,
                self.quote_side.asset_decimals,
            ),
        };
        daily_limit_from_liquidity_ema(liquidity_ema, asset_decimals, limit_bps)
    }
}
