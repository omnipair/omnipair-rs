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
        self.refresh_risk_book()
    }

    pub fn recompute_market_health_from_risk_book(&mut self) -> Result<()> {
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
        self.recompute_market_health_from_risk_book()
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

        collateral_value_from_pessimistic_reserves_nad(
            collateral_side.reserve_ledger.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserve_ledger.live_reserve,
            debt_side.asset_decimals,
            collateral_amount,
            price_ema_nad,
            directional_price_ema_nad,
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

        collateral_amount_for_debt_amount_ceil(
            collateral_side.reserve_ledger.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserve_ledger.live_reserve,
            debt_side.asset_decimals,
            debt_with_incentive,
            price_ema_nad,
            directional_price_ema_nad,
        )
    }

    fn collateral_amount_for_debt_value_cap_with_risk(
        &self,
        debt_asset: MarketAsset,
        debt_value_nad: u128,
        risk_book: &RiskBook,
    ) -> Result<u64> {
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

        collateral_amount_for_debt_value_floor(
            collateral_side.reserve_ledger.live_reserve,
            collateral_side.asset_decimals,
            debt_side.reserve_ledger.live_reserve,
            debt_side.asset_decimals,
            debt_value_nad,
            price_ema_nad,
            directional_price_ema_nad,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::NAD,
        state::{BufferLedger, DebtBook, MarginPosition, MarketConfig, MarketSide, ReserveLedger},
    };

    fn market_side(asset_mint: Pubkey) -> MarketSide {
        MarketSide {
            asset_mint,
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: ReserveLedger {
                live_reserve: 1_000_000,
                cash_reserve: 1_000_000,
                reserved_liability: 0,
            },
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn market_config() -> MarketConfig {
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

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint),
            market_side(quote_mint),
            market_config(),
            [21_u8; 32],
            42,
            250,
        )
        .unwrap()
    }

    fn margin_position() -> MarginPosition {
        MarginPosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            base_collateral: 0,
            quote_collateral: 0,
            recognized_base_collateral_for_quote_debt: 0,
            recognized_quote_collateral_for_base_debt: 0,
            fixed_base_debt_shares: 0,
            fixed_quote_debt_shares: 0,
            bump: 1,
        }
    }

    #[test]
    fn risk_book_refresh_recomputes_stored_market_health() {
        let mut market = test_market();
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000, market.debt_book.base_borrow_index_nad).unwrap();
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 150_000;
        market.health.effective_base_debt_nad = 1;
        market.health.base_debt_health_bps = 1;

        market.refresh_risk_book().unwrap();

        assert_eq!(market.health.effective_base_debt_nad, 100_000_000);
        assert!(market.health.base_debt_health_bps > 1);
    }

    #[test]
    fn market_health_uses_recognized_collateral_not_idle_inventory() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(1_000, NAD as u128).unwrap();

        market.refresh_market_health().unwrap();
        assert_eq!(market.health.base_debt_health_bps, 0);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealth)
        );

        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 1_500;
        market.refresh_market_health().unwrap();
        assert!(market.health.base_debt_health_bps >= 14_900);
        market.assert_market_health().unwrap();
    }

    #[test]
    fn market_health_rejects_raw_unit_decimal_pump() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = market_side(base_mint);
        let mut quote_side = market_side(quote_mint);
        base_side.asset_decimals = 6;
        quote_side.asset_decimals = 9;
        base_side.reserve_ledger.live_reserve = 1_000_000_000;
        quote_side.reserve_ledger.live_reserve = 1_000_000_000_000_000;
        let mut market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            base_side,
            quote_side,
            MarketConfig {
                recognized_collateral_cap_bps: 11_000,
                ..market_config()
            },
            [8_u8; 32],
            42,
            253,
        )
        .unwrap();
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(900_000_000, NAD as u128).unwrap();
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 1_000_000_000;
        market.refresh_market_health().unwrap();

        assert!(market.health.base_debt_health_bps < market.config.market_health_min_bps as u64);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealth)
        );
    }

    #[test]
    fn daily_borrow_limit_uses_side_liquidity_ema() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000;
        market.refresh_risk_book().unwrap();

        market
            .enforce_daily_borrow_limit(MarketAsset::Base, 200_000)
            .unwrap();
        let err = market
            .enforce_daily_borrow_limit(MarketAsset::Base, 1)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::DailyLimitExceeded));
        assert_eq!(market.base_side.daily_limit_book.borrowed_bucket, 200_000);
    }

    #[test]
    fn daily_limit_rejects_zero_liquidity() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 0;
        market.quote_side.reserve_ledger.live_reserve = 0;

        let err = market
            .enforce_daily_borrow_limit(MarketAsset::Base, 1)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientLiquidity));
    }

    #[test]
    fn circuit_breaker_rejects_spot_ema_divergence() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000;
        market.quote_side.reserve_ledger.live_reserve = 2_000_000;
        market.risk_book.base_price_ema_nad = NAD;
        market.risk_book.quote_price_ema_nad = NAD;

        let err = market.assert_spot_ema_divergence().unwrap_err();

        assert_eq!(err, error!(ErrorCode::MarketRiskCircuitBreaker));
    }

    #[test]
    fn circuit_breaker_rejects_k_ema_drawdown() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 900_000;
        market.quote_side.reserve_ledger.live_reserve = 900_000;
        market.risk_book.base_price_ema_nad = NAD;
        market.risk_book.quote_price_ema_nad = NAD;
        market.risk_book.k_ema = normalize_to_nad(1_000_000, market.base_side.asset_decimals)
            .unwrap()
            .checked_mul(normalize_to_nad(1_000_000, market.quote_side.asset_decimals).unwrap())
            .unwrap();

        market.assert_spot_ema_divergence().unwrap();
        let err = market.assert_risk_circuit_breakers().unwrap_err();

        assert_eq!(err, error!(ErrorCode::MarketRiskCircuitBreaker));
    }

    #[test]
    fn effective_debt_applies_gamma_only_to_hedged_overlay() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 2_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 2_000_000_000;
        market.refresh_risk_book().unwrap();
        market.config.effective_debt_weight_min_bps = 5_000;
        market.config.effective_debt_gamma_nad = 2 * NAD;
        market.risk_book.liquidity_ema = 1_000 * NAD as u128;
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, NAD as u128).unwrap();
        market
            .quote_side
            .claim_token_ledger
            .hedged_claim_token_supply = 100_000_000;

        let raw_hedged_debt = market.hedged_base_debt_nad(&market.risk_book).unwrap();
        let effective_hedged_debt = effective_hedged_debt_nad(
            raw_hedged_debt,
            market.risk_book.liquidity_ema,
            market.config.effective_debt_weight_min_bps,
            market.config.effective_debt_gamma_nad,
        )
        .unwrap();
        let effective = market.effective_base_debt_nad().unwrap();

        assert!(raw_hedged_debt > 0);
        assert!(effective_hedged_debt < raw_hedged_debt);
        assert_eq!(effective, 100 * NAD as u128 + effective_hedged_debt);
    }

    #[test]
    fn recognized_collateral_is_capped_by_debt_value() {
        let mut market = test_market();
        market.config.recognized_collateral_cap_bps = 15_000;
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.refresh_risk_book().unwrap();
        let mut position = margin_position();
        position.quote_collateral = 1_000_000_000;
        position.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, market.debt_book.base_borrow_index_nad).unwrap();

        let recognized = market
            .debt_capped_recognized_collateral(&position, MarketAsset::Base, &market.risk_book)
            .unwrap();
        let recognized_value = market
            .collateral_value_nad(MarketAsset::Quote, recognized, &market.risk_book)
            .unwrap();
        let debt_value_cap = normalize_to_nad(100_000_000, market.base_side.asset_decimals)
            .unwrap()
            .checked_mul(15_000)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .unwrap();

        assert!(recognized > 100_000_000);
        assert!(recognized < position.quote_collateral);
        assert!(recognized_value <= debt_value_cap);
    }

    #[test]
    fn recognition_cap_rejects_idle_collateral_pump() {
        let mut market = test_market();
        market.config.recognized_collateral_cap_bps = 15_000;
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.refresh_risk_book().unwrap();
        let mut position = margin_position();
        position.quote_collateral = 1_000_000_000;
        position.recognized_quote_collateral_for_base_debt = 1_000_000_000;
        position.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, market.debt_book.base_borrow_index_nad).unwrap();

        let err = market
            .assert_recognition_cap(&position, MarketAsset::Base)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientRecognizedCollateral));
    }

    #[test]
    fn stale_recognition_cannot_exceed_margin_collateral() {
        let mut position = margin_position();
        position.base_collateral = 100;
        position.recognized_base_collateral_for_quote_debt = 80;
        assert_eq!(position.idle_base_collateral().unwrap(), 20);

        position.recognized_base_collateral_for_quote_debt = 101;
        assert_eq!(
            position.idle_base_collateral().unwrap_err(),
            error!(ErrorCode::InsufficientRecognizedCollateral)
        );
    }
}
