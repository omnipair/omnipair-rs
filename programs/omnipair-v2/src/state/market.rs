use anchor_lang::prelude::*;

use super::{
    DebtBook, InsuranceReserve, MarginPosition, MarketAsset, MarketConfig, MarketHealth,
    MarketSide, RecognitionLedger, RiskBook,
};
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::math::*;
use crate::shared::math::ceil_div;

#[account]
#[derive(InitSpace)]
pub struct Market {
    pub version: u8,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub base_side: MarketSide,
    pub quote_side: MarketSide,
    pub config: MarketConfig,
    pub debt_book: DebtBook,
    pub risk_book: RiskBook,
    pub health: MarketHealth,
    pub recognition_ledger: RecognitionLedger,
    pub insurance_reserve: InsuranceReserve,
    pub params_hash: [u8; 32],
    pub last_update_slot: u64,
    pub reduce_only: bool,
    pub bump: u8,
}

impl Market {
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        base_mint: Pubkey,
        quote_mint: Pubkey,
        operator: Pubkey,
        manager: Pubkey,
        base_side: MarketSide,
        quote_side: MarketSide,
        config: MarketConfig,
        params_hash: [u8; 32],
        current_slot: u64,
        bump: u8,
    ) -> Result<Self> {
        config.validate()?;
        require_keys_neq!(base_mint, quote_mint, ErrorCode::InvalidMint);
        require_keys_neq!(operator, Pubkey::default(), ErrorCode::InvalidMarketConfig);
        require_keys_neq!(manager, Pubkey::default(), ErrorCode::InvalidMarketConfig);
        require_keys_eq!(base_mint, base_side.asset_mint, ErrorCode::InvalidMint);
        require_keys_eq!(quote_mint, quote_side.asset_mint, ErrorCode::InvalidMint);

        Ok(Self {
            version: MARKET_VERSION,
            base_mint,
            quote_mint,
            operator,
            manager,
            base_side,
            quote_side,
            config,
            debt_book: DebtBook {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                ..DebtBook::default()
            },
            risk_book: RiskBook {
                last_snapshot_slot: current_slot,
                ..RiskBook::default()
            },
            health: MarketHealth::default(),
            recognition_ledger: RecognitionLedger {
                last_recognition_slot: current_slot,
                ..RecognitionLedger::default()
            },
            insurance_reserve: InsuranceReserve::default(),
            params_hash,
            last_update_slot: current_slot,
            reduce_only: false,
            bump,
        })
    }

    pub fn assert_live(&self) -> Result<()> {
        self.assert_started()?;
        require!(!self.reduce_only, ErrorCode::MarketReduceOnly);
        Ok(())
    }

    pub fn assert_started(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= self.config.start_time, ErrorCode::MarketNotStarted);
        Ok(())
    }

    pub fn side(&self, market_asset: MarketAsset) -> Result<&MarketSide> {
        match market_asset {
            MarketAsset::Base => Ok(&self.base_side),
            MarketAsset::Quote => Ok(&self.quote_side),
        }
    }

    pub fn side_mut(&mut self, market_asset: MarketAsset) -> Result<&mut MarketSide> {
        match market_asset {
            MarketAsset::Base => Ok(&mut self.base_side),
            MarketAsset::Quote => Ok(&mut self.quote_side),
        }
    }

    pub fn swap_sides(&self, asset_in: MarketAsset) -> (&MarketSide, &MarketSide) {
        match asset_in {
            MarketAsset::Base => (&self.base_side, &self.quote_side),
            MarketAsset::Quote => (&self.quote_side, &self.base_side),
        }
    }

    pub fn swap_sides_mut(&mut self, asset_in: MarketAsset) -> (&mut MarketSide, &mut MarketSide) {
        match asset_in {
            MarketAsset::Base => (&mut self.base_side, &mut self.quote_side),
            MarketAsset::Quote => (&mut self.quote_side, &mut self.base_side),
        }
    }

    pub fn assert_market_invariants(&self) -> Result<()> {
        self.base_side.assert_claim_coverage()?;
        self.quote_side.assert_claim_coverage()?;
        self.base_side.fee_ledger.assert_backed()?;
        self.quote_side.fee_ledger.assert_backed()?;
        Ok(())
    }

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

    pub fn apply_buffer_ratio_update(&mut self, buffer_ratio_bps: u16) -> Result<()> {
        self.assert_buffer_ratio_change_unlocked(buffer_ratio_bps)?;
        let required_buffer0 = self
            .base_side
            .assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        let required_buffer1 = self
            .quote_side
            .assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        self.base_side
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer0);
        self.quote_side
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer1);
        Ok(())
    }

    fn assert_buffer_ratio_change_unlocked(&self, buffer_ratio_bps: u16) -> Result<()> {
        if buffer_ratio_bps == self.base_side.buffer_ledger.buffer_ratio_bps
            && buffer_ratio_bps == self.quote_side.buffer_ledger.buffer_ratio_bps
        {
            return Ok(());
        }
        require!(
            self.base_side.claim_token_ledger.staked_claim_token_supply == 0
                && self.quote_side.claim_token_ledger.staked_claim_token_supply == 0
                && self.base_side.buffer_ledger.staked_buffer_share_amount == 0
                && self.quote_side.buffer_ledger.staked_buffer_share_amount == 0
                && self.base_side.fee_ledger.fee_liability == 0
                && self.quote_side.fee_ledger.fee_liability == 0,
            ErrorCode::InvalidMarketConfig
        );
        Ok(())
    }
}

impl Market {
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

    fn hedged_base_debt_nad(&self, risk_book: &RiskBook) -> Result<u128> {
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

    fn collateral_value_nad(
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

#[macro_export]
macro_rules! generate_market_seeds {
    ($market:expr) => {
        [
            MARKET_V2_SEED_PREFIX,
            $market.base_mint.as_ref(),
            $market.quote_mint.as_ref(),
            $market.params_hash.as_ref(),
            &[$market.bump],
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::{
            BufferLedger, DailyLimitBook, FeeLedger, HedgePosition, MarginPosition,
            MarketFeeClaimKind, StakePosition,
        },
        transitions::fee::{CarryForwardStakerFees, RecordFeeCredit},
        transitions::reserve::{AddLiquidity, RemoveLiquidity},
    };

    fn test_market_side(asset_mint: Pubkey, buffer_ratio_bps: u16) -> MarketSide {
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
            buffer_ledger: BufferLedger {
                buffer_ratio_bps,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
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
            test_market_side(base_mint, 2_000),
            test_market_side(quote_mint, 2_000),
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
            },
            [7_u8; 32],
            42,
            254,
        )
        .unwrap()
    }

    fn stake_position() -> StakePosition {
        StakePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            available_buffer_share_amount: 0,
            staked_claim_token_amount: 0,
            staked_buffer_share_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    fn hedge_position() -> HedgePosition {
        HedgePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_token_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
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
    fn market_initialize_preserves_creator_chosen_base_quote_order() {
        let base_mint = Pubkey::new_from_array([2_u8; 32]);
        let quote_mint = Pubkey::new_from_array([1_u8; 32]);
        let market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            test_market_side(base_mint, 2_000),
            test_market_side(quote_mint, 2_000),
            test_market().config,
            [7_u8; 32],
            42,
            254,
        )
        .unwrap();

        assert_eq!(market.base_mint, base_mint);
        assert_eq!(market.quote_mint, quote_mint);
        assert_eq!(market.base_side.asset_mint, base_mint);
        assert_eq!(market.quote_side.asset_mint, quote_mint);
    }

    #[test]
    fn market_initialize_rejects_default_authorities() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let base_side = test_market_side(base_mint, 2_000);
        let quote_side = test_market_side(quote_mint, 2_000);
        let config = test_market().config;

        let default_operator = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::default(),
            Pubkey::new_unique(),
            base_side,
            quote_side,
            config,
            [7_u8; 32],
            42,
            254,
        )
        .err()
        .unwrap();
        let default_manager = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::default(),
            base_side,
            quote_side,
            config,
            [7_u8; 32],
            42,
            254,
        )
        .err()
        .unwrap();

        assert_eq!(default_operator, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(default_manager, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn market_config_rejects_soft_borrow_until_implemented() {
        let mut config = test_market().config;
        config.soft_borrow_enabled = true;

        let err = config.validate().unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn market_config_rejects_recognition_cap_below_health_floor() {
        let mut config = test_market().config;
        config.recognized_collateral_cap_bps = 10_000;
        config.market_health_min_bps = 11_000;

        let err = config.validate().unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn market_config_rejects_inert_ema_half_lives() {
        let mut config = test_market().config;
        config.ema_half_life_ms = 0;
        assert_eq!(
            config.validate().unwrap_err(),
            error!(ErrorCode::InvalidMarketConfig)
        );

        let mut config = test_market().config;
        config.directional_ema_half_life_ms = MIN_HALF_LIFE_MS - 1;
        assert_eq!(
            config.validate().unwrap_err(),
            error!(ErrorCode::InvalidMarketConfig)
        );

        let mut config = test_market().config;
        config.k_ema_half_life_ms = MAX_HALF_LIFE_MS + 1;
        assert_eq!(
            config.validate().unwrap_err(),
            error!(ErrorCode::InvalidMarketConfig)
        );
    }

    #[test]
    fn market_config_rejects_inert_fee_routing() {
        let mut config = test_market().config;
        config.fee_routing_k_nad = 0;

        let err = config.validate().unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn market_config_rejects_invalid_k_drawdown_limit() {
        let mut config = test_market().config;
        config.k_ema_drawdown_bps = BPS_DENOMINATOR + 1;

        let err = config.validate().unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn reserve_deposit_mints_claim_minus_buffer() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);

        let receipt = AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(receipt.claim_amount, 800_000);
        assert_eq!(receipt.buffer_amount, 200_000);
        assert_eq!(market_side.reserve_ledger.live_reserve, 1_000_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 1_000_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            800_000
        );
        assert_eq!(market_side.buffer_ledger.buffer_share_supply, 200_000);
        assert_eq!(market_side.buffer_ledger.required_buffer, 200_000);
        assert_eq!(market_side.claim_floor().unwrap(), 1_000_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn claim_redemption_is_fixed_one_to_one_principal() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();
        RecordFeeCredit::new(10_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        RemoveLiquidity::new(100_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.reserve_ledger.live_reserve, 900_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 900_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            700_000
        );
        assert_eq!(market_side.buffer_ledger.required_buffer, 175_000);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 10_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 10_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn fee_ledger_allocates_only_to_matched_stake() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 100_000;

        RecordFeeCredit::new(1_000, 1_000, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side.fee_ledger.operator_fee_liability, 100);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 900);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 1_800_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn fee_ledger_routes_pressure_share_to_hedged_liability() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.reserve_ledger.live_reserve = 1_000_000;
        market_side.claim_token_ledger.protected_claim_token_supply = 800_000;
        market_side.claim_token_ledger.hedged_claim_token_supply = 200_000;
        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 200_000;

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 0);
        assert!(market_side.fee_ledger.hedged_fee_liability > 0);
        assert!(market_side.fee_ledger.hedged_fee_growth_index_nad > 0);
        assert!(market_side.fee_ledger.fee_liability < 1_000);
        assert_eq!(
            market_side.fee_ledger.total_liability().unwrap(),
            market_side.fee_ledger.fee_vault_balance
        );
    }

    #[test]
    fn unstaked_claims_do_not_receive_fee_growth() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.claim_token_ledger.protected_claim_token_supply = 800_000;

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn unallocated_fees_carry_forward_to_next_active_stake() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();
        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1_000);

        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 200_000;
        CarryForwardStakerFees.apply(&mut market_side).unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 1_000_000);
        assert_eq!(market_side.fee_ledger.fee_liability, 1_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn unallocated_fee_rounding_dust_stays_carried_forward() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.claim_token_ledger.staked_claim_token_supply = 1_600_000_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 400_000_000;

        RecordFeeCredit::new(1, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn market_fee_liabilities_settle_operator_and_protocol_buckets() {
        let mut fee_ledger = FeeLedger {
            fee_vault_balance: 700,
            operator_fee_liability: 400,
            protocol_fee_liability: 300,
            ..FeeLedger::default()
        };

        let operator_fee = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap();
        let protocol_fee = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Protocol)
            .unwrap();
        let err = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap_err();

        assert_eq!(operator_fee, 400);
        assert_eq!(protocol_fee, 300);
        assert_eq!(fee_ledger.operator_fee_liability, 0);
        assert_eq!(fee_ledger.protocol_fee_liability, 0);
        assert_eq!(err, error!(ErrorCode::AmountZero));
    }

    #[test]
    fn stake_position_accrues_checkpointed_non_compounding_fees() {
        let mut position = stake_position();
        position.credit_buffer_share_amount(200_000).unwrap();
        position.stake(800_000, 200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 3 * NAD as u128);

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
    }

    #[test]
    fn hedge_position_tracks_one_to_one_nav_without_stake_rights() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        let mut position = hedge_position();

        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_add(500_000)
            .unwrap();
        position.increase(500_000).unwrap();

        assert_eq!(position.hedged_claim_token_amount, 500_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            500_000
        );
        assert_eq!(market_side.claim_token_ledger.staked_claim_token_supply, 0);
        assert_eq!(market_side.buffer_ledger.staked_buffer_share_amount, 0);

        position.decrease(125_000).unwrap();
        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_sub(125_000)
            .unwrap();
        assert_eq!(position.hedged_claim_token_amount, 375_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            375_000
        );
    }

    #[test]
    fn hedge_position_accrues_checkpointed_routed_fees() {
        let mut position = hedge_position();
        position.increase(200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 4 * NAD as u128);

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
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
        let mut base_side = test_market_side(base_mint, 2_000);
        let mut quote_side = test_market_side(quote_mint, 2_000);
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
                ..test_market().config
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
    fn buffer_ratio_update_recomputes_required_floor() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(2_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market.base_side.buffer_ledger.buffer_share_supply += 100_000;
        market.base_side.reserve_ledger.live_reserve += 100_000;
        market.quote_side.buffer_ledger.buffer_share_supply += 200_000;
        market.quote_side.reserve_ledger.live_reserve += 200_000;

        market.apply_buffer_ratio_update(2_500).unwrap();

        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_500);
        assert_eq!(market.base_side.buffer_ledger.required_buffer, 266_667);
        assert_eq!(market.quote_side.buffer_ledger.required_buffer, 533_334);
    }

    #[test]
    fn buffer_ratio_update_rejects_uncovered_floor() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();

        let err = market.apply_buffer_ratio_update(2_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientBufferShares));
        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_000);
        assert_eq!(market.base_side.buffer_ledger.required_buffer, 200_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_active_stake() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market
            .base_side
            .claim_token_ledger
            .staked_claim_token_supply = 800_000;
        market.base_side.buffer_ledger.staked_buffer_share_amount = 200_000;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_staker_fee_liability() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market.quote_side.fee_ledger.fee_liability = 1;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(market.quote_side.buffer_ledger.buffer_ratio_bps, 2_000);
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
    fn daily_limit_bucket_decays_over_one_day() {
        let mut book = DailyLimitBook {
            borrowed_bucket: 100_000,
            withdrawn_bucket: 50_000,
            last_decay_slot: 0,
        };
        let half_day_slots = MS_PER_DAY / TARGET_MS_PER_SLOT / 2;

        book.decay_to_slot(half_day_slots).unwrap();

        assert_eq!(book.borrowed_bucket, 50_000);
        assert_eq!(book.withdrawn_bucket, 25_000);
    }

    #[test]
    fn daily_limit_rejects_zero_liquidity() {
        let mut market = test_market();

        let err = market
            .enforce_daily_borrow_limit(MarketAsset::Base, 1)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientLiquidity));
    }

    #[test]
    fn risk_book_bootstraps_cached_spot_observation() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = test_market_side(base_mint, 2_000);
        let mut quote_side = test_market_side(quote_mint, 2_000);
        base_side.reserve_ledger.live_reserve = 1_000_000;
        quote_side.reserve_ledger.live_reserve = 2_000_000;

        let refreshed = RiskBook::default()
            .refreshed(&base_side, &quote_side, &test_market().config, 42)
            .unwrap();

        assert_eq!(refreshed.base_price_ema_nad, 2 * NAD);
        assert_eq!(refreshed.quote_price_ema_nad, NAD / 2);
        assert_eq!(refreshed.cached_spot_base_price_nad, 2 * NAD);
        assert_eq!(refreshed.cached_spot_quote_price_nad, NAD / 2);
        assert_eq!(refreshed.last_snapshot_slot, 42);
    }

    #[test]
    fn risk_book_rolls_ema_from_cached_spot_not_current_spot() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = test_market_side(base_mint, 2_000);
        let mut quote_side = test_market_side(quote_mint, 2_000);
        base_side.reserve_ledger.live_reserve = 1_000_000;
        quote_side.reserve_ledger.live_reserve = 2_000_000;
        let risk_book = RiskBook {
            base_price_ema_nad: NAD,
            quote_price_ema_nad: NAD,
            directional_base_price_ema_nad: NAD,
            directional_quote_price_ema_nad: NAD,
            cached_spot_base_price_nad: NAD,
            cached_spot_quote_price_nad: NAD,
            last_snapshot_slot: 0,
            ..RiskBook::default()
        };

        let refreshed = risk_book
            .refreshed(&base_side, &quote_side, &test_market().config, 10_000)
            .unwrap();

        assert_eq!(refreshed.base_price_ema_nad, NAD);
        assert_eq!(refreshed.quote_price_ema_nad, NAD);
        assert_eq!(refreshed.directional_base_price_ema_nad, NAD);
        assert_eq!(refreshed.directional_quote_price_ema_nad, NAD);
        assert_eq!(refreshed.cached_spot_base_price_nad, 2 * NAD);
        assert_eq!(refreshed.cached_spot_quote_price_nad, NAD / 2);
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
