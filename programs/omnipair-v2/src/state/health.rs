use anchor_lang::prelude::*;

use super::{MarginPosition, Market, MarketAsset, MarketHealth, RiskBook};
use crate::{
    constants::{BPS_DENOMINATOR, LIQUIDATION_INCENTIVE_BPS},
    errors::ErrorCode,
    math::*,
    shared::math::ceil_div,
};

impl Market {
    pub fn refresh_market_health(&mut self) -> Result<()> {
        self.refresh_risk_book()?;
        let effective_base_debt_nad = self.effective_base_debt_nad()?;
        let effective_quote_debt_nad = self.effective_quote_debt_nad()?;
        let quote_collateral_value_for_base_debt_nad = self
            .quote_collateral_value_for_base_debt_nad(
                self.recognition_ledger
                    .debt_bearing_quote_collateral_for_base_debt,
            )?;
        let base_collateral_value_for_quote_debt_nad = self
            .base_collateral_value_for_quote_debt_nad(
                self.recognition_ledger
                    .debt_bearing_base_collateral_for_quote_debt,
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
                .recognition_ledger
                .debt_bearing_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: self
                .recognition_ledger
                .debt_bearing_quote_collateral_for_base_debt,
            effective_base_debt_nad,
            effective_quote_debt_nad,
            base_debt_health_bps,
            quote_debt_health_bps,
        };
        Ok(())
    }

    pub fn current_risk_book(&self) -> Result<RiskBook> {
        let current_slot = Clock::get()
            .map(|clock| clock.slot)
            .unwrap_or(self.last_update_slot);
        self.risk_book.refreshed(
            &self.base_side,
            &self.quote_side,
            &self.config,
            current_slot,
        )
    }

    pub fn refresh_risk_book(&mut self) -> Result<()> {
        self.risk_book = self.current_risk_book()?;
        self.last_update_slot = self.risk_book.last_snapshot_slot;
        Ok(())
    }

    pub fn enforce_daily_borrow_limit(
        &mut self,
        market_asset: MarketAsset,
        amount: u64,
    ) -> Result<()> {
        self.refresh_risk_book()?;
        let current_slot = self.risk_book.last_snapshot_slot;
        let limit = self.daily_limit_for_side(market_asset, self.config.max_daily_borrow_bps)?;
        self.side_mut(market_asset)?
            .daily_limit_book
            .record_borrow(amount, limit, current_slot)
    }

    pub fn enforce_daily_withdraw_limit(
        &mut self,
        market_asset: MarketAsset,
        amount: u64,
    ) -> Result<()> {
        self.refresh_risk_book()?;
        let current_slot = self.risk_book.last_snapshot_slot;
        let limit = self.daily_limit_for_side(market_asset, self.config.max_daily_withdraw_bps)?;
        self.side_mut(market_asset)?
            .daily_limit_book
            .record_withdraw(amount, limit, current_slot)
    }

    pub fn assert_spot_ema_divergence(&self) -> Result<()> {
        assert_price_divergence(
            market_spot_price_nad(&self.base_side, &self.quote_side)?,
            self.risk_book.base_price_ema_nad,
            self.config.spot_ema_divergence_bps,
        )?;
        assert_price_divergence(
            market_spot_price_nad(&self.quote_side, &self.base_side)?,
            self.risk_book.quote_price_ema_nad,
            self.config.spot_ema_divergence_bps,
        )
    }

    pub fn assert_risk_circuit_breakers(&self) -> Result<()> {
        self.assert_spot_ema_divergence()?;
        self.assert_k_ema_drawdown()
    }

    pub fn assert_k_ema_drawdown(&self) -> Result<()> {
        if self.risk_book.k_ema == 0 {
            return Ok(());
        }
        assert_k_drawdown(
            market_k_nad(&self.base_side, &self.quote_side)?,
            self.risk_book.k_ema,
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
        self.collateral_value_nad(MarketAsset::Quote, quote_collateral_amount, &self.risk_book)
    }

    pub fn base_collateral_value_for_quote_debt_nad(
        &self,
        base_collateral_amount: u64,
    ) -> Result<u128> {
        self.collateral_value_nad(MarketAsset::Base, base_collateral_amount, &self.risk_book)
    }

    pub fn collateral_amount_for_debt_value(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
    ) -> Result<u64> {
        self.collateral_amount_for_debt_value_with_risk(
            debt_asset,
            debt_amount,
            &self.current_risk_book()?,
        )
    }

    pub fn debt_capped_recognized_collateral(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        risk_book: &RiskBook,
    ) -> Result<u64> {
        let cap_bps = self.config.recognized_collateral_cap_bps as u128;
        let (fixed_debt, debt_decimals, total_collateral) = match debt_asset {
            MarketAsset::Base => (
                margin_position.fixed_base_debt(&self.debt_book)?,
                self.base_side.asset_decimals,
                margin_position.quote_collateral,
            ),
            MarketAsset::Quote => (
                margin_position.fixed_quote_debt(&self.debt_book)?,
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
            risk_book,
        )?;
        Ok(total_collateral.min(capped_collateral))
    }

    pub fn position_health_bps(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
    ) -> Result<u64> {
        let risk_book = self.current_risk_book()?;
        match debt_asset {
            MarketAsset::Base => health_bps(
                self.collateral_value_nad(
                    MarketAsset::Quote,
                    margin_position.recognized_quote_collateral_for_base_debt,
                    &risk_book,
                )?,
                normalize_to_nad(
                    margin_position.fixed_base_debt(&self.debt_book)?,
                    self.base_side.asset_decimals,
                )?,
            ),
            MarketAsset::Quote => health_bps(
                self.collateral_value_nad(
                    MarketAsset::Base,
                    margin_position.recognized_base_collateral_for_quote_debt,
                    &risk_book,
                )?,
                normalize_to_nad(
                    margin_position.fixed_quote_debt(&self.debt_book)?,
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
        let risk_book = self.current_risk_book()?;
        let max_recognized =
            self.debt_capped_recognized_collateral(margin_position, debt_asset, &risk_book)?;
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
        let (fixed_debt, soft_debt, hedged_debt_nad, debt_side) = match debt_asset {
            MarketAsset::Base => (
                self.debt_book.fixed_base_debt()?,
                self.debt_book.soft_base_debt()?,
                self.hedged_base_debt_nad(&self.risk_book)?,
                &self.base_side,
            ),
            MarketAsset::Quote => (
                self.debt_book.fixed_quote_debt()?,
                self.debt_book.soft_quote_debt()?,
                self.hedged_quote_debt_nad(&self.risk_book)?,
                &self.quote_side,
            ),
        };
        let fixed_debt_nad = normalize_to_nad(fixed_debt, debt_side.asset_decimals)?;
        let soft_debt_nad = normalize_to_nad(soft_debt, debt_side.asset_decimals)?;
        let hedged_debt_nad = effective_hedged_debt_nad(
            hedged_debt_nad,
            self.risk_book.liquidity_ema,
            self.config.effective_debt_weight_min_bps,
            self.config.effective_debt_gamma_nad,
        )?;

        fixed_debt_nad
            .checked_add(soft_debt_nad)
            .and_then(|value| value.checked_add(hedged_debt_nad))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub(crate) fn hedged_base_debt_nad(&self, risk_book: &RiskBook) -> Result<u128> {
        self.collateral_value_nad(
            MarketAsset::Quote,
            self.quote_side.claim_token_ledger.hedged_claim_token_supply,
            risk_book,
        )
    }

    fn hedged_quote_debt_nad(&self, risk_book: &RiskBook) -> Result<u128> {
        self.collateral_value_nad(
            MarketAsset::Base,
            self.base_side.claim_token_ledger.hedged_claim_token_supply,
            risk_book,
        )
    }

    pub(crate) fn collateral_value_nad(
        &self,
        collateral_asset: MarketAsset,
        collateral_amount: u64,
        risk_book: &RiskBook,
    ) -> Result<u128> {
        if collateral_amount == 0 {
            return Ok(0);
        }
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match collateral_asset {
                MarketAsset::Base => (
                    &self.base_side,
                    &self.quote_side,
                    risk_book.base_price_ema_nad,
                    risk_book.directional_base_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.quote_side,
                    &self.base_side,
                    risk_book.quote_price_ema_nad,
                    risk_book.directional_quote_price_ema_nad,
                ),
            };
        let collateral_reserve = normalize_to_nad(
            collateral_side.reserve_ledger.live_reserve as u128,
            collateral_side.asset_decimals,
        )?;
        let debt_reserve = normalize_to_nad(
            debt_side.reserve_ledger.live_reserve as u128,
            debt_side.asset_decimals,
        )?;
        let collateral_amount =
            normalize_to_nad(collateral_amount as u128, collateral_side.asset_decimals)?;
        let (collateral_virtual_reserve, debt_virtual_reserve) =
            construct_normalized_virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                price_ema_nad,
                directional_price_ema_nad,
            )?;
        calculate_normalized_amount_out(
            collateral_virtual_reserve,
            debt_virtual_reserve,
            collateral_amount,
        )
    }

    fn collateral_amount_for_debt_value_with_risk(
        &self,
        debt_asset: MarketAsset,
        debt_amount: u64,
        risk_book: &RiskBook,
    ) -> Result<u64> {
        let debt_with_incentive = ceil_div(
            (debt_amount as u128)
                .checked_mul((BPS_DENOMINATOR + LIQUIDATION_INCENTIVE_BPS) as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::MarketMathOverflow)?;
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match debt_asset {
                MarketAsset::Base => (
                    &self.quote_side,
                    &self.base_side,
                    risk_book.quote_price_ema_nad,
                    risk_book.directional_quote_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.base_side,
                    &self.quote_side,
                    risk_book.base_price_ema_nad,
                    risk_book.directional_base_price_ema_nad,
                ),
            };
        let collateral_reserve = normalize_to_nad(
            collateral_side.reserve_ledger.live_reserve as u128,
            collateral_side.asset_decimals,
        )?;
        let debt_reserve = normalize_to_nad(
            debt_side.reserve_ledger.live_reserve as u128,
            debt_side.asset_decimals,
        )?;
        let debt_amount_nad = normalize_to_nad(debt_with_incentive, debt_side.asset_decimals)?;
        let (collateral_virtual_reserve, debt_virtual_reserve) =
            construct_normalized_virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                price_ema_nad,
                directional_price_ema_nad,
            )?;
        let collateral_amount_nad = calculate_normalized_amount_in(
            collateral_virtual_reserve,
            debt_virtual_reserve,
            debt_amount_nad,
        )?;
        denormalize_from_nad_ceil(collateral_amount_nad, collateral_side.asset_decimals)
    }

    fn collateral_amount_for_debt_value_cap_with_risk(
        &self,
        debt_asset: MarketAsset,
        debt_value_nad: u128,
        risk_book: &RiskBook,
    ) -> Result<u64> {
        if debt_value_nad == 0 {
            return Ok(0);
        }
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            match debt_asset {
                MarketAsset::Base => (
                    &self.quote_side,
                    &self.base_side,
                    risk_book.quote_price_ema_nad,
                    risk_book.directional_quote_price_ema_nad,
                ),
                MarketAsset::Quote => (
                    &self.base_side,
                    &self.quote_side,
                    risk_book.base_price_ema_nad,
                    risk_book.directional_base_price_ema_nad,
                ),
            };
        let collateral_reserve = normalize_to_nad(
            collateral_side.reserve_ledger.live_reserve as u128,
            collateral_side.asset_decimals,
        )?;
        let debt_reserve = normalize_to_nad(
            debt_side.reserve_ledger.live_reserve as u128,
            debt_side.asset_decimals,
        )?;
        let (collateral_virtual_reserve, debt_virtual_reserve) =
            construct_normalized_virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                price_ema_nad,
                directional_price_ema_nad,
            )?;
        let collateral_amount_nad = calculate_normalized_amount_in_floor(
            collateral_virtual_reserve,
            debt_virtual_reserve,
            debt_value_nad,
        )?;
        denormalize_from_nad_floor(collateral_amount_nad, collateral_side.asset_decimals)
    }

    fn daily_limit_for_side(&self, market_asset: MarketAsset, limit_bps: u16) -> Result<u64> {
        let (liquidity_ema, asset_decimals) = match market_asset {
            MarketAsset::Base => (
                self.risk_book.base_liquidity_ema,
                self.base_side.asset_decimals,
            ),
            MarketAsset::Quote => (
                self.risk_book.quote_liquidity_ema,
                self.quote_side.asset_decimals,
            ),
        };
        require!(liquidity_ema > 0, ErrorCode::InsufficientLiquidity);
        let limit_nad = liquidity_ema
            .checked_mul(limit_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        denormalize_from_nad_floor(limit_nad, asset_decimals)
    }
}
