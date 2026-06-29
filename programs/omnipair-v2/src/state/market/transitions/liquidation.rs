use anchor_lang::prelude::*;

use crate::{
    constants::{
        BPS_DENOMINATOR, LIQUIDATION_INCENTIVE_BPS, LIQUIDATION_INSURANCE_FUNDING_BPS,
        LIQUIDATION_MAX_INCENTIVE_BPS, NAD,
    },
    errors::ErrorCode,
    math::{denormalize_from_nad_ceil, normalize_to_nad},
    shared::math::ceil_div,
    state::{Debt, MarginPosition, Market, MarketAsset},
};

pub struct Liquidation {
    pub debt_asset: MarketAsset,
    pub repay_credit: u64,
    pub insurance_spent: u64,
    pub insurance_credit: u64,
    pub max_socialized_loss: u64,
    pub terms: LiquidationTerms,
    pub pricing: LiquidationPricing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidationPricing {
    PessimisticReserves,
    ReferencePrice { debt_per_collateral_price_nad: u64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiquidationTerms {
    pub liquidation_incentive_bps: u16,
    pub insurance_funding_bps: u16,
    pub total_penalty_bps: u16,
    pub max_repay_amount: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiquidationReceipt {
    pub repaid_amount: u64,
    pub interest_paid: u64,
    pub collateral_seized: u64,
    pub collateral_to_liquidator: u64,
    pub insurance_funded: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
    pub liquidation_incentive_bps: u16,
    pub insurance_funding_bps: u16,
    pub max_repay_amount: u64,
}

impl Liquidation {
    #[cfg(test)]
    pub fn new(
        debt_asset: MarketAsset,
        repay_credit: u64,
        insurance_spent: u64,
        insurance_credit: u64,
        max_socialized_loss: u64,
        terms: LiquidationTerms,
    ) -> Self {
        Self {
            debt_asset,
            repay_credit,
            insurance_spent,
            insurance_credit,
            max_socialized_loss,
            terms,
            pricing: LiquidationPricing::PessimisticReserves,
        }
    }

    pub fn new_with_pricing(
        debt_asset: MarketAsset,
        repay_credit: u64,
        insurance_spent: u64,
        insurance_credit: u64,
        max_socialized_loss: u64,
        terms: LiquidationTerms,
        pricing: LiquidationPricing,
    ) -> Self {
        Self {
            debt_asset,
            repay_credit,
            insurance_spent,
            insurance_credit,
            max_socialized_loss,
            terms,
            pricing,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<LiquidationReceipt> {
        let debt_before = position_debt(market, margin_position, self.debt_asset)?;
        require_gte!(
            debt_before,
            self.repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        require_gte!(
            self.terms.max_repay_amount,
            self.repay_credit,
            ErrorCode::LiquidationRepayTooLarge
        );
        let collateral_before = position_collateral(margin_position, self.debt_asset);
        let collateral_seized = collateral_to_seize(
            market,
            self.debt_asset,
            self.repay_credit,
            collateral_before,
            self.terms.total_penalty_bps,
            self.pricing,
        )?;
        let collateral_to_liquidator = collateral_to_liquidator(
            market,
            self.debt_asset,
            self.repay_credit,
            collateral_seized,
            self.terms.liquidation_incentive_bps,
            self.pricing,
        )?;
        let insurance_funded = collateral_seized
            .checked_sub(collateral_to_liquidator)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let collateral_exhausted = collateral_seized == collateral_before;
        let repay_plus_insurance = (self.repay_credit as u128)
            .checked_add(self.insurance_credit as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(
            debt_before,
            repay_plus_insurance,
            ErrorCode::InsufficientDebt
        );
        let cap_remaining = self
            .terms
            .max_repay_amount
            .checked_sub(self.repay_credit)
            .ok_or(ErrorCode::LiquidationRepayTooLarge)?;
        require_gte!(
            cap_remaining,
            self.insurance_credit,
            ErrorCode::LiquidationRepayTooLarge
        );

        let bad_debt = debt_before
            .checked_sub(repay_plus_insurance)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let socialized_loss = if collateral_exhausted {
            u64::try_from(bad_debt).map_err(|_| ErrorCode::MarketMathOverflow)?
        } else {
            0
        };
        require_gte!(
            self.max_socialized_loss,
            socialized_loss,
            ErrorCode::LiquidationSocializationExceeded
        );
        if bad_debt > 0 && !collateral_exhausted {
            require!(socialized_loss == 0, ErrorCode::InsufficientInsurance);
        }

        let debt_reduction = repay_plus_insurance
            .checked_add(socialized_loss as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let cash_repaid =
            u64::try_from(repay_plus_insurance).map_err(|_| ErrorCode::MarketMathOverflow)?;
        let debt_reduction_u64 =
            u64::try_from(debt_reduction).map_err(|_| ErrorCode::MarketMathOverflow)?;
        // Track the principal/interest split for cash-backed repayment without
        // treating socialized loss as received interest.
        let interest_paid = market.debt.realize_margin_liquidation(
            self.debt_asset,
            cash_repaid,
            debt_reduction_u64,
        )?;
        let principal_credit = cash_repaid
            .checked_sub(interest_paid)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        apply_liquidation_debt_reduction(
            market,
            margin_position,
            self.debt_asset,
            debt_reduction,
            collateral_seized,
        )?;

        {
            let debt_side = market.side_mut(self.debt_asset)?;
            debt_side.reserves.live_reserve = debt_side
                .reserves
                .live_reserve
                .checked_add(principal_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
            debt_side.reserves.cash_reserve = debt_side
                .reserves
                .cash_reserve
                .checked_add(principal_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
        }
        match self.debt_asset {
            MarketAsset::Base => {
                market.insurance.base_available = market
                    .insurance
                    .base_available
                    .checked_sub(self.insurance_spent)
                    .ok_or(ErrorCode::InsufficientInsurance)?;
                market.insurance.quote_available = market
                    .insurance
                    .quote_available
                    .checked_add(insurance_funded)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                market.insurance.quote_available = market
                    .insurance
                    .quote_available
                    .checked_sub(self.insurance_spent)
                    .ok_or(ErrorCode::InsufficientInsurance)?;
                market.insurance.base_available = market
                    .insurance
                    .base_available
                    .checked_add(insurance_funded)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }

        margin_position.record_risk_update()?;
        market.recompute_market_health_from_risk()?;
        market.assert_risk_circuit_breakers()?;
        Ok(LiquidationReceipt {
            repaid_amount: self.repay_credit,
            interest_paid,
            collateral_seized,
            collateral_to_liquidator,
            insurance_funded,
            insurance_drawn: self.insurance_credit,
            socialized_loss,
            remaining_debt: position_debt(market, margin_position, self.debt_asset)?,
            liquidation_incentive_bps: self.terms.liquidation_incentive_bps,
            insurance_funding_bps: self.terms.insurance_funding_bps,
            max_repay_amount: self.terms.max_repay_amount,
        })
    }
}

pub(crate) fn insurance_request_for_liquidation_with_terms_and_pricing(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    repay_credit: u64,
    max_insurance_draw: u64,
    terms: LiquidationTerms,
    pricing: LiquidationPricing,
) -> Result<u64> {
    let debt_before = position_debt(market, margin_position, debt_asset)?;
    require_gte!(
        debt_before,
        repay_credit as u128,
        ErrorCode::InsufficientDebt
    );
    require_gte!(
        terms.max_repay_amount,
        repay_credit,
        ErrorCode::LiquidationRepayTooLarge
    );
    let collateral_before = position_collateral(margin_position, debt_asset);
    let collateral_seized = collateral_to_seize(
        market,
        debt_asset,
        repay_credit,
        collateral_before,
        terms.total_penalty_bps,
        pricing,
    )?;
    let remaining_debt = debt_before
        .checked_sub(repay_credit as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if collateral_seized < collateral_before || remaining_debt == 0 {
        return Ok(0);
    }
    let available = match debt_asset {
        MarketAsset::Base => market.insurance.base_available,
        MarketAsset::Quote => market.insurance.quote_available,
    };
    let remaining_debt_cap = u64::try_from(remaining_debt).unwrap_or(u64::MAX);
    let remaining_partial_cap = terms
        .max_repay_amount
        .checked_sub(repay_credit)
        .ok_or(ErrorCode::LiquidationRepayTooLarge)?;
    Ok(remaining_debt_cap
        .min(available)
        .min(max_insurance_draw)
        .min(remaining_partial_cap))
}

pub(crate) fn liquidation_terms(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
) -> Result<LiquidationTerms> {
    let health_before = market.position_health_bps(margin_position, debt_asset)?;
    let liquidation_incentive_bps =
        liquidation_incentive_bps(health_before, market.config.market_health_min_bps as u64);
    let insurance_funding_bps =
        liquidation_insurance_funding_bps(liquidation_incentive_bps, &market.config)?;
    let total_penalty_bps = liquidation_incentive_bps
        .checked_add(insurance_funding_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let max_repay_amount =
        max_repay_to_restore_health(market, margin_position, debt_asset, total_penalty_bps)?;
    Ok(LiquidationTerms {
        liquidation_incentive_bps,
        insurance_funding_bps,
        total_penalty_bps,
        max_repay_amount,
    })
}

pub(crate) fn liquidation_terms_with_incentive_and_pricing(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    liquidation_incentive_bps: u16,
    pricing: LiquidationPricing,
) -> Result<LiquidationTerms> {
    let max_incentive_bps = liquidation_max_incentive_bps(
        market.position_health_bps(margin_position, debt_asset)?,
        market.config.market_health_min_bps as u64,
    );
    require_gte!(
        max_incentive_bps,
        liquidation_incentive_bps,
        ErrorCode::InvalidLiquidationAuction
    );
    let insurance_funding_bps =
        liquidation_insurance_funding_bps(liquidation_incentive_bps, &market.config)?;
    let total_penalty_bps = liquidation_incentive_bps
        .checked_add(insurance_funding_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let max_repay_amount = max_repay_to_restore_health_with_pricing(
        market,
        margin_position,
        debt_asset,
        total_penalty_bps,
        pricing,
    )?;
    Ok(LiquidationTerms {
        liquidation_incentive_bps,
        insurance_funding_bps,
        total_penalty_bps,
        max_repay_amount,
    })
}

fn apply_liquidation_debt_reduction(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset: MarketAsset,
    debt_reduction: u128,
    collateral_seized: u64,
) -> Result<()> {
    match debt_asset {
        MarketAsset::Base => {
            let shares_before = margin_position.fixed_base_shares;
            let debt_before = margin_position.fixed_base_debt(&market.debt)?;
            let shares_to_burn = shares_to_burn_for_reduction(
                debt_reduction,
                debt_before,
                shares_before,
                market.debt.base_borrow_index_nad,
            )?;
            margin_position.quote_collateral = margin_position
                .quote_collateral
                .checked_sub(collateral_seized)
                .ok_or(ErrorCode::InsufficientRecognizedCollateral)?;
            let recognized_decrease = recognized_decrease_after_seizure(
                margin_position.recognized_quote_collateral_for_base_debt,
                margin_position.quote_collateral,
                shares_to_burn,
                shares_before,
            )?;
            margin_position.recognized_quote_collateral_for_base_debt = margin_position
                .recognized_quote_collateral_for_base_debt
                .checked_sub(recognized_decrease)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            margin_position.fixed_base_shares = margin_position
                .fixed_base_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt.fixed_base_shares = market
                .debt
                .fixed_base_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt.recognized_quote_collateral_for_base_debt = market
                .debt
                .recognized_quote_collateral_for_base_debt
                .checked_sub(recognized_decrease)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        MarketAsset::Quote => {
            let shares_before = margin_position.fixed_quote_shares;
            let debt_before = margin_position.fixed_quote_debt(&market.debt)?;
            let shares_to_burn = shares_to_burn_for_reduction(
                debt_reduction,
                debt_before,
                shares_before,
                market.debt.quote_borrow_index_nad,
            )?;
            margin_position.base_collateral = margin_position
                .base_collateral
                .checked_sub(collateral_seized)
                .ok_or(ErrorCode::InsufficientRecognizedCollateral)?;
            let recognized_decrease = recognized_decrease_after_seizure(
                margin_position.recognized_base_collateral_for_quote_debt,
                margin_position.base_collateral,
                shares_to_burn,
                shares_before,
            )?;
            margin_position.recognized_base_collateral_for_quote_debt = margin_position
                .recognized_base_collateral_for_quote_debt
                .checked_sub(recognized_decrease)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            margin_position.fixed_quote_shares = margin_position
                .fixed_quote_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt.fixed_quote_shares = market
                .debt
                .fixed_quote_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt.recognized_base_collateral_for_quote_debt = market
                .debt
                .recognized_base_collateral_for_quote_debt
                .checked_sub(recognized_decrease)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
    }
    Ok(())
}

fn position_debt(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
) -> Result<u128> {
    match debt_asset {
        MarketAsset::Base => margin_position.fixed_base_debt(&market.debt),
        MarketAsset::Quote => margin_position.fixed_quote_debt(&market.debt),
    }
}

fn position_collateral(margin_position: &MarginPosition, debt_asset: MarketAsset) -> u64 {
    match debt_asset {
        MarketAsset::Base => margin_position.quote_collateral,
        MarketAsset::Quote => margin_position.base_collateral,
    }
}

fn collateral_to_seize(
    market: &Market,
    debt_asset: MarketAsset,
    repay_credit: u64,
    collateral_before: u64,
    total_penalty_bps: u16,
    pricing: LiquidationPricing,
) -> Result<u64> {
    let seizure = collateral_amount_for_debt_value_with_pricing(
        market,
        debt_asset,
        repay_credit,
        total_penalty_bps,
        pricing,
    )?;
    Ok(seizure.min(collateral_before))
}

fn collateral_to_liquidator(
    market: &Market,
    debt_asset: MarketAsset,
    repay_credit: u64,
    collateral_seized: u64,
    liquidation_incentive_bps: u16,
    pricing: LiquidationPricing,
) -> Result<u64> {
    let liquidator_collateral = collateral_amount_for_debt_value_with_pricing(
        market,
        debt_asset,
        repay_credit,
        liquidation_incentive_bps,
        pricing,
    )?;
    Ok(liquidator_collateral.min(collateral_seized))
}

pub(crate) fn liquidation_incentive_bps(health_bps: u64, min_health_bps: u64) -> u16 {
    liquidation_max_incentive_bps(health_bps, min_health_bps)
}

pub(crate) fn liquidation_max_incentive_bps(health_bps: u64, min_health_bps: u64) -> u16 {
    let shortfall = min_health_bps.saturating_sub(health_bps);
    let max_for_config = min_health_bps
        .saturating_sub(BPS_DENOMINATOR as u64 + 1)
        .min(LIQUIDATION_MAX_INCENTIVE_BPS as u64);
    shortfall
        .max(LIQUIDATION_INCENTIVE_BPS as u64)
        .min(max_for_config) as u16
}

pub(crate) fn liquidation_insurance_funding_bps(
    liquidation_incentive_bps: u16,
    config: &crate::state::MarketConfig,
) -> Result<u16> {
    let max_total_penalty =
        (config.market_health_min_bps as u64).saturating_sub(BPS_DENOMINATOR as u64 + 1);
    let remaining = max_total_penalty.saturating_sub(liquidation_incentive_bps as u64);
    Ok(LIQUIDATION_INSURANCE_FUNDING_BPS.min(u16::try_from(remaining).unwrap_or(u16::MAX)))
}

pub(crate) fn max_repay_to_restore_health(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    total_penalty_bps: u16,
) -> Result<u64> {
    max_repay_to_restore_health_with_pricing(
        market,
        margin_position,
        debt_asset,
        total_penalty_bps,
        LiquidationPricing::PessimisticReserves,
    )
}

fn max_repay_to_restore_health_with_pricing(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    total_penalty_bps: u16,
    pricing: LiquidationPricing,
) -> Result<u64> {
    let debt_before = position_debt(market, margin_position, debt_asset)?;
    let debt_decimals = match debt_asset {
        MarketAsset::Base => market.base_side.asset_decimals,
        MarketAsset::Quote => market.quote_side.asset_decimals,
    };
    let debt_value_nad = normalize_to_nad(debt_before, debt_decimals)?;
    let collateral_value_nad =
        recognized_collateral_value_with_pricing(market, margin_position, debt_asset, pricing)?;
    let target_bps = market.config.market_health_min_bps as u128;
    let penalty_multiplier_bps = (BPS_DENOMINATOR as u128)
        .checked_add(total_penalty_bps as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require!(
        target_bps > penalty_multiplier_bps,
        ErrorCode::InvalidMarketConfig
    );
    let target_debt_value = debt_value_nad
        .checked_mul(target_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let recognized_collateral_value = collateral_value_nad
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if target_debt_value <= recognized_collateral_value {
        return Ok(0);
    }
    let shortfall_value = target_debt_value
        .checked_sub(recognized_collateral_value)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let denominator = target_bps
        .checked_sub(penalty_multiplier_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let repay_value_nad =
        ceil_div(shortfall_value, denominator).ok_or(ErrorCode::MarketMathOverflow)?;
    let repay_amount = denormalize_from_nad_ceil(repay_value_nad, debt_decimals)?;
    Ok(repay_amount.min(u64::try_from(debt_before).unwrap_or(u64::MAX)))
}

fn recognized_collateral_value_with_pricing(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    pricing: LiquidationPricing,
) -> Result<u128> {
    match pricing {
        LiquidationPricing::PessimisticReserves => {
            let risk = market.current_risk()?;
            match debt_asset {
                MarketAsset::Base => market.collateral_value_nad(
                    MarketAsset::Quote,
                    margin_position.recognized_quote_collateral_for_base_debt,
                    &risk,
                ),
                MarketAsset::Quote => market.collateral_value_nad(
                    MarketAsset::Base,
                    margin_position.recognized_base_collateral_for_quote_debt,
                    &risk,
                ),
            }
        }
        LiquidationPricing::ReferencePrice {
            debt_per_collateral_price_nad,
        } => {
            let collateral_asset = debt_asset.opposite();
            let collateral_amount = match debt_asset {
                MarketAsset::Base => margin_position.recognized_quote_collateral_for_base_debt,
                MarketAsset::Quote => margin_position.recognized_base_collateral_for_quote_debt,
            };
            collateral_value_at_reference_price_nad(
                market,
                collateral_asset,
                collateral_amount,
                debt_per_collateral_price_nad,
            )
        }
    }
}

fn collateral_value_at_reference_price_nad(
    market: &Market,
    collateral_asset: MarketAsset,
    collateral_amount: u64,
    debt_per_collateral_price_nad: u64,
) -> Result<u128> {
    require!(
        debt_per_collateral_price_nad > 0,
        ErrorCode::InvalidSettlementPrice
    );
    let collateral_decimals = market.side(collateral_asset)?.asset_decimals;
    let collateral_amount_nad = normalize_to_nad(collateral_amount as u128, collateral_decimals)?;
    collateral_amount_nad
        .checked_mul(debt_per_collateral_price_nad as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn collateral_amount_for_debt_value_with_pricing(
    market: &Market,
    debt_asset: MarketAsset,
    debt_amount: u64,
    penalty_bps: u16,
    pricing: LiquidationPricing,
) -> Result<u64> {
    match pricing {
        LiquidationPricing::PessimisticReserves => market
            .collateral_amount_for_debt_value_with_penalty_bps(
                debt_asset,
                debt_amount,
                penalty_bps,
            ),
        LiquidationPricing::ReferencePrice {
            debt_per_collateral_price_nad,
        } => collateral_amount_for_debt_value_at_reference_price(
            market,
            debt_asset,
            debt_amount,
            penalty_bps,
            debt_per_collateral_price_nad,
        ),
    }
}

fn collateral_amount_for_debt_value_at_reference_price(
    market: &Market,
    debt_asset: MarketAsset,
    debt_amount: u64,
    penalty_bps: u16,
    debt_per_collateral_price_nad: u64,
) -> Result<u64> {
    require!(
        debt_per_collateral_price_nad > 0,
        ErrorCode::InvalidSettlementPrice
    );
    let debt_decimals = market.side(debt_asset)?.asset_decimals;
    let collateral_decimals = market.side(debt_asset.opposite())?.asset_decimals;
    let debt_with_penalty = ceil_div(
        (debt_amount as u128)
            .checked_mul((BPS_DENOMINATOR + penalty_bps) as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflow)?;
    let debt_value_nad = normalize_to_nad(debt_with_penalty, debt_decimals)?;
    let collateral_amount_nad = ceil_div(
        debt_value_nad
            .checked_mul(NAD as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        debt_per_collateral_price_nad as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflow)?;
    denormalize_from_nad_ceil(collateral_amount_nad, collateral_decimals)
}

fn shares_to_burn_for_reduction(
    debt_reduction: u128,
    debt_before: u128,
    shares_before: u128,
    borrow_index_nad: u128,
) -> Result<u128> {
    require!(
        shares_before > 0 && debt_before > 0,
        ErrorCode::InsufficientDebt
    );
    if debt_reduction >= debt_before {
        return Ok(shares_before);
    }
    let debt_reduction =
        u64::try_from(debt_reduction).map_err(|_| ErrorCode::MarketMathOverflow)?;
    Debt::debt_to_shares(debt_reduction, borrow_index_nad).map(|shares| shares.min(shares_before))
}

fn recognized_decrease_after_seizure(
    recognized_before: u64,
    collateral_after: u64,
    shares_to_burn: u128,
    shares_before: u128,
) -> Result<u64> {
    if shares_to_burn == shares_before {
        return Ok(recognized_before);
    }
    let proportional = (recognized_before as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let proportional = u64::try_from(proportional).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let recognized_after_proportional = recognized_before
        .checked_sub(proportional)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if recognized_after_proportional <= collateral_after {
        Ok(proportional)
    } else {
        let extra = recognized_after_proportional
            .checked_sub(collateral_after)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        proportional
            .checked_add(extra)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }
}

#[cfg(test)]
mod tests {
    include!("../../../tests/transitions/liquidation.rs");
}
