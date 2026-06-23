use anchor_lang::prelude::*;

use super::{MarketConfig, MarketSide};
use crate::math::{
    directional_ema_u64, ema_u128, ema_u64, market_k_nad, market_liquidity_nad,
    market_spot_price_nad, normalize_to_nad, observed_or_current_u128, observed_or_current_u64,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Risk {
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

impl Risk {
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
            base_side.reserves.live_reserve as u128,
            base_side.asset_decimals,
        )?;
        let current_quote_liquidity_nad = normalize_to_nad(
            quote_side.reserves.live_reserve as u128,
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
