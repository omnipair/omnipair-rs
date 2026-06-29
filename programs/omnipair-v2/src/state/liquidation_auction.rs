use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MARKET_VERSION},
    errors::ErrorCode,
    state::{MarginPosition, Market, MarketAsset},
};

pub(crate) use crate::state::market::transitions::liquidation::LiquidationPricing;

#[account]
#[derive(Default, InitSpace)]
pub struct LiquidationAuction {
    pub version: u8,
    pub active: bool,
    pub market: Pubkey,
    pub margin_position: Pubkey,
    pub borrower: Pubkey,
    pub debt_asset: u8,
    pub collateral_asset: u8,
    pub debt_mint: Pubkey,
    pub collateral_mint: Pubkey,
    pub position_risk_epoch: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    pub start_health_bps: u64,
    pub start_incentive_bps: u16,
    pub max_incentive_bps: u16,
    pub max_repay_amount: u64,
    pub reference_price_nad: u64,
    pub settled_repay_amount: u64,
    pub last_settlement_slot: u64,
    pub bump: u8,
}

pub struct OpenLiquidationAuctionParams {
    pub market: Pubkey,
    pub margin_position: Pubkey,
    pub borrower: Pubkey,
    pub debt_asset: MarketAsset,
    pub debt_mint: Pubkey,
    pub collateral_mint: Pubkey,
    pub position_risk_epoch: u64,
    pub current_slot: u64,
    pub duration_slots: u64,
    pub start_health_bps: u64,
    pub start_incentive_bps: u16,
    pub max_incentive_bps: u16,
    pub max_repay_amount: u64,
    pub reference_price_nad: u64,
    pub bump: u8,
}

impl LiquidationAuction {
    pub fn can_open_for(&self, margin_position: &MarginPosition) -> bool {
        !self.active || self.position_risk_epoch != margin_position.risk_epoch
    }

    pub fn open(&mut self, params: OpenLiquidationAuctionParams) -> Result<()> {
        require!(params.duration_slots > 0, ErrorCode::InvalidMarketConfig);
        require!(
            params.start_incentive_bps <= params.max_incentive_bps,
            ErrorCode::InvalidLiquidationAuction
        );
        require!(
            params.reference_price_nad > 0,
            ErrorCode::InvalidSettlementPrice
        );
        let end_slot = params
            .current_slot
            .checked_add(params.duration_slots)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.version = MARKET_VERSION;
        self.active = true;
        self.market = params.market;
        self.margin_position = params.margin_position;
        self.borrower = params.borrower;
        self.debt_asset = params.debt_asset.code();
        self.collateral_asset = params.debt_asset.opposite().code();
        self.debt_mint = params.debt_mint;
        self.collateral_mint = params.collateral_mint;
        self.position_risk_epoch = params.position_risk_epoch;
        self.start_slot = params.current_slot;
        self.end_slot = end_slot;
        self.start_health_bps = params.start_health_bps;
        self.start_incentive_bps = params.start_incentive_bps;
        self.max_incentive_bps = params.max_incentive_bps;
        self.max_repay_amount = params.max_repay_amount;
        self.reference_price_nad = params.reference_price_nad;
        self.settled_repay_amount = 0;
        self.last_settlement_slot = 0;
        self.bump = params.bump;
        Ok(())
    }

    pub fn assert_matches(
        &self,
        market_key: Pubkey,
        margin_position_key: Pubkey,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        debt_mint: Pubkey,
        collateral_mint: Pubkey,
    ) -> Result<()> {
        require!(self.active, ErrorCode::LiquidationAuctionInactive);
        require_keys_eq!(
            self.market,
            market_key,
            ErrorCode::InvalidLiquidationAuction
        );
        require_keys_eq!(
            self.margin_position,
            margin_position_key,
            ErrorCode::InvalidLiquidationAuction
        );
        require_keys_eq!(
            self.borrower,
            margin_position.owner,
            ErrorCode::InvalidLiquidationAuction
        );
        require_eq!(
            self.debt_asset,
            debt_asset.code(),
            ErrorCode::InvalidLiquidationAuction
        );
        require_keys_eq!(
            self.debt_mint,
            debt_mint,
            ErrorCode::InvalidLiquidationAuction
        );
        require_keys_eq!(
            self.collateral_mint,
            collateral_mint,
            ErrorCode::InvalidLiquidationAuction
        );
        require_eq!(
            self.position_risk_epoch,
            margin_position.risk_epoch,
            ErrorCode::StaleLiquidationAuction
        );
        Ok(())
    }

    pub fn current_incentive_bps(
        &self,
        current_slot: u64,
        live_max_incentive_bps: u16,
    ) -> Result<u16> {
        let max_incentive = self.max_incentive_bps.min(live_max_incentive_bps);
        let start_incentive = self.start_incentive_bps.min(max_incentive);
        if max_incentive <= start_incentive {
            return Ok(max_incentive);
        }
        let duration = self.end_slot.saturating_sub(self.start_slot).max(1);
        let elapsed = current_slot.saturating_sub(self.start_slot).min(duration);
        let spread = (max_incentive as u128)
            .checked_sub(start_incentive as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let accrued = spread
            .checked_mul(elapsed as u128)
            .and_then(|value| value.checked_div(duration as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let incentive = (start_incentive as u128)
            .checked_add(accrued)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        u16::try_from(incentive).map_err(|_| ErrorCode::MarketMathOverflow.into())
    }

    pub fn record_settlement(
        &mut self,
        margin_position: &MarginPosition,
        repaid_amount: u64,
        current_slot: u64,
        still_liquidatable: bool,
    ) -> Result<()> {
        self.settled_repay_amount = self
            .settled_repay_amount
            .checked_add(repaid_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.position_risk_epoch = margin_position.risk_epoch;
        self.last_settlement_slot = current_slot;
        self.active = still_liquidatable;
        Ok(())
    }
}

pub fn liquidation_auction_reference_price_nad(
    market: &Market,
    debt_asset: MarketAsset,
) -> Result<u64> {
    let price = match debt_asset {
        MarketAsset::Base => market.risk.quote_price_ema_nad,
        MarketAsset::Quote => market.risk.base_price_ema_nad,
    };
    require!(price > 0, ErrorCode::InvalidSettlementPrice);
    Ok(price)
}

pub fn liquidation_auction_start_incentive_bps(
    configured_start_incentive_bps: u16,
    max_incentive_bps: u16,
) -> Result<u16> {
    require_gte!(
        BPS_DENOMINATOR,
        configured_start_incentive_bps,
        ErrorCode::InvalidMarketConfig
    );
    Ok(configured_start_incentive_bps.min(max_incentive_bps))
}

impl Market {
    pub fn liquidation_terms(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
    ) -> Result<crate::state::market::transitions::liquidation::LiquidationTerms> {
        crate::state::market::transitions::liquidation::liquidation_terms(
            self,
            margin_position,
            debt_asset,
        )
    }

    pub fn liquidation_terms_with_incentive_and_pricing(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        liquidation_incentive_bps: u16,
        pricing: crate::state::market::transitions::liquidation::LiquidationPricing,
    ) -> Result<crate::state::market::transitions::liquidation::LiquidationTerms> {
        crate::state::market::transitions::liquidation::liquidation_terms_with_incentive_and_pricing(
            self,
            margin_position,
            debt_asset,
            liquidation_incentive_bps,
            pricing,
        )
    }

    pub fn insurance_request_for_liquidation_with_terms_and_pricing(
        &self,
        margin_position: &MarginPosition,
        debt_asset: MarketAsset,
        repay_credit: u64,
        max_insurance_draw: u64,
        terms: crate::state::market::transitions::liquidation::LiquidationTerms,
        pricing: crate::state::market::transitions::liquidation::LiquidationPricing,
    ) -> Result<u64> {
        crate::state::market::transitions::liquidation::insurance_request_for_liquidation_with_terms_and_pricing(
            self,
            margin_position,
            debt_asset,
            repay_credit,
            max_insurance_draw,
            terms,
            pricing,
        )
    }

    pub fn settle_liquidation(
        &mut self,
        margin_position: &mut MarginPosition,
        debt_asset: MarketAsset,
        repay_credit: u64,
        insurance_spent: u64,
        insurance_credit: u64,
        max_socialized_loss: u64,
        terms: crate::state::market::transitions::liquidation::LiquidationTerms,
        pricing: crate::state::market::transitions::liquidation::LiquidationPricing,
    ) -> Result<crate::state::market::transitions::liquidation::LiquidationReceipt> {
        crate::state::market::transitions::liquidation::Liquidation::new_with_pricing(
            debt_asset,
            repay_credit,
            insurance_spent,
            insurance_credit,
            max_socialized_loss,
            terms,
            pricing,
        )
        .apply(self, margin_position)
    }
}

#[cfg(test)]
mod tests {
    include!("../tests/state/liquidation_auction.rs");
}
