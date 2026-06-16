use anchor_lang::prelude::*;

use super::{MarketConfig, MarketSide};
use crate::{
    constants::NAD,
    errors::ErrorCode,
    math::{
        directional_ema_u64, ema_u128, ema_u64, market_k_nad, market_liquidity_nad,
        market_spot_price_nad, normalize_to_nad, observed_or_current_u128, observed_or_current_u64,
    },
    shared::math::ceil_div,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DebtBook {
    pub fixed_base_debt_shares: u128,
    pub fixed_quote_debt_shares: u128,
    pub soft_base_debt_shares: u128,
    pub soft_quote_debt_shares: u128,
    pub base_borrow_index_nad: u128,
    pub quote_borrow_index_nad: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RiskBook {
    pub base_price_ema_nad: u64,
    pub quote_price_ema_nad: u64,
    pub directional_base_price_ema_nad: u64,
    pub directional_quote_price_ema_nad: u64,
    pub cached_spot_base_price_nad: u64,
    pub cached_spot_quote_price_nad: u64,
    pub cached_k_nad: u128,
    pub cached_liquidity_nad: u128,
    pub cached_base_liquidity_nad: u128,
    pub cached_quote_liquidity_nad: u128,
    pub k_ema: u128,
    pub liquidity_ema: u128,
    pub base_liquidity_ema: u128,
    pub quote_liquidity_ema: u128,
    pub last_snapshot_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketHealth {
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub effective_base_debt_nad: u128,
    pub effective_quote_debt_nad: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
}

impl DebtBook {
    pub fn debt_to_shares(amount: u64, borrow_index_nad: u128) -> Result<u128> {
        require!(amount > 0, ErrorCode::AmountZero);
        ceil_div(
            (amount as u128)
                .checked_mul(NAD as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            borrow_index_nad,
        )
        .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn shares_to_debt(shares: u128, borrow_index_nad: u128) -> Result<u128> {
        shares
            .checked_mul(borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn fixed_base_debt(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_base_debt_shares, self.base_borrow_index_nad)
    }

    pub fn fixed_quote_debt(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_quote_debt_shares, self.quote_borrow_index_nad)
    }

    pub fn soft_base_debt(&self) -> Result<u128> {
        self.soft_base_debt_shares
            .checked_mul(self.base_borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn soft_quote_debt(&self) -> Result<u128> {
        self.soft_quote_debt_shares
            .checked_mul(self.quote_borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_base_debt(&self) -> Result<u128> {
        self.fixed_base_debt()?
            .checked_add(self.soft_base_debt()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_quote_debt(&self) -> Result<u128> {
        self.fixed_quote_debt()?
            .checked_add(self.soft_quote_debt()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }
}

impl RiskBook {
    pub fn refreshed(
        &self,
        base_side: &MarketSide,
        quote_side: &MarketSide,
        config: &MarketConfig,
        current_slot: u64,
    ) -> Result<Self> {
        let current_base_price_nad = market_spot_price_nad(base_side, quote_side)?;
        let current_quote_price_nad = market_spot_price_nad(quote_side, base_side)?;
        let current_base_liquidity_nad = normalize_to_nad(
            base_side.reserve_ledger.live_reserve as u128,
            base_side.asset_decimals,
        )?;
        let current_quote_liquidity_nad = normalize_to_nad(
            quote_side.reserve_ledger.live_reserve as u128,
            quote_side.asset_decimals,
        )?;
        let current_liquidity_nad = market_liquidity_nad(base_side, quote_side)?;
        let current_k_nad = market_k_nad(base_side, quote_side)?;

        let cached_spot_base_price_nad =
            observed_or_current_u64(self.cached_spot_base_price_nad, current_base_price_nad);
        let cached_spot_quote_price_nad =
            observed_or_current_u64(self.cached_spot_quote_price_nad, current_quote_price_nad);
        let cached_base_liquidity_nad =
            observed_or_current_u128(self.cached_base_liquidity_nad, current_base_liquidity_nad);
        let cached_quote_liquidity_nad =
            observed_or_current_u128(self.cached_quote_liquidity_nad, current_quote_liquidity_nad);
        let cached_liquidity_nad =
            observed_or_current_u128(self.cached_liquidity_nad, current_liquidity_nad);
        let cached_k_nad = observed_or_current_u128(self.cached_k_nad, current_k_nad);

        let base_price_ema_nad = ema_u64(
            self.base_price_ema_nad,
            cached_spot_base_price_nad,
            self.last_snapshot_slot,
            current_slot,
            config.ema_half_life_ms,
        );
        let quote_price_ema_nad = ema_u64(
            self.quote_price_ema_nad,
            cached_spot_quote_price_nad,
            self.last_snapshot_slot,
            current_slot,
            config.ema_half_life_ms,
        );
        let directional_base_price_ema_nad = directional_ema_u64(
            self.directional_base_price_ema_nad,
            cached_spot_base_price_nad,
            self.last_snapshot_slot,
            current_slot,
            config.directional_ema_half_life_ms,
        );
        let directional_quote_price_ema_nad = directional_ema_u64(
            self.directional_quote_price_ema_nad,
            cached_spot_quote_price_nad,
            self.last_snapshot_slot,
            current_slot,
            config.directional_ema_half_life_ms,
        );
        let liquidity_ema = ema_u128(
            self.liquidity_ema,
            cached_liquidity_nad,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let k_ema = ema_u128(
            self.k_ema,
            cached_k_nad,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let base_liquidity_ema = ema_u128(
            self.base_liquidity_ema,
            cached_base_liquidity_nad,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let quote_liquidity_ema = ema_u128(
            self.quote_liquidity_ema,
            cached_quote_liquidity_nad,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );

        Ok(Self {
            base_price_ema_nad,
            quote_price_ema_nad,
            directional_base_price_ema_nad,
            directional_quote_price_ema_nad,
            cached_spot_base_price_nad: current_base_price_nad,
            cached_spot_quote_price_nad: current_quote_price_nad,
            cached_k_nad: current_k_nad,
            cached_liquidity_nad: current_liquidity_nad,
            cached_base_liquidity_nad: current_base_liquidity_nad,
            cached_quote_liquidity_nad: current_quote_liquidity_nad,
            k_ema,
            liquidity_ema,
            base_liquidity_ema,
            quote_liquidity_ema,
            last_snapshot_slot: current_slot,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::BufferLedger,
    };

    fn market_side(asset_mint: Pubkey, buffer_ratio_bps: u16) -> MarketSide {
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
            effective_debt_weight_min_bps: BPS_DENOMINATOR,
            effective_debt_gamma_nad: NAD,
            soft_borrow_enabled: false,
            hedged_lp_enabled: true,
            start_time: 0,
        }
    }

    #[test]
    fn risk_book_bootstraps_cached_spot_observation() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = market_side(base_mint, 2_000);
        let mut quote_side = market_side(quote_mint, 2_000);
        base_side.reserve_ledger.live_reserve = 1_000_000;
        quote_side.reserve_ledger.live_reserve = 2_000_000;

        let refreshed = RiskBook::default()
            .refreshed(&base_side, &quote_side, &market_config(), 42)
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
        let mut base_side = market_side(base_mint, 2_000);
        let mut quote_side = market_side(quote_mint, 2_000);
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
            .refreshed(&base_side, &quote_side, &market_config(), 10_000)
            .unwrap();

        assert_eq!(refreshed.base_price_ema_nad, NAD);
        assert_eq!(refreshed.quote_price_ema_nad, NAD);
        assert_eq!(refreshed.directional_base_price_ema_nad, NAD);
        assert_eq!(refreshed.directional_quote_price_ema_nad, NAD);
        assert_eq!(refreshed.cached_spot_base_price_nad, 2 * NAD);
        assert_eq!(refreshed.cached_spot_quote_price_nad, NAD / 2);
    }
}
