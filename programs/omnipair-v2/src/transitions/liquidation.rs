use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{Debt, MarginPosition, Market, MarketAsset},
};

pub struct Liquidation {
    pub debt_asset: MarketAsset,
    pub repay_credit: u64,
    pub insurance_spent: u64,
    pub insurance_credit: u64,
    pub max_socialized_loss: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiquidationReceipt {
    pub repaid_amount: u64,
    pub collateral_seized: u64,
    pub collateral_to_liquidator: u64,
    pub insurance_funded: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
}

impl Liquidation {
    pub fn new(
        debt_asset: MarketAsset,
        repay_credit: u64,
        insurance_spent: u64,
        insurance_credit: u64,
        max_socialized_loss: u64,
    ) -> Self {
        Self {
            debt_asset,
            repay_credit,
            insurance_spent,
            insurance_credit,
            max_socialized_loss,
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
        let collateral_before = position_collateral(margin_position, self.debt_asset);
        let collateral_seized = collateral_to_seize(
            market,
            self.debt_asset,
            self.repay_credit,
            collateral_before,
        )?;
        let collateral_to_liquidator = collateral_to_liquidator(
            market,
            self.debt_asset,
            self.repay_credit,
            collateral_seized,
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
        let _interest_paid = market.debt.realize_margin_liquidation(
            self.debt_asset,
            cash_repaid,
            debt_reduction_u64,
        )?;
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
                .checked_add(self.repay_credit)
                .and_then(|value| value.checked_add(self.insurance_credit))
                .ok_or(ErrorCode::ReserveOverflow)?;
            debt_side.reserves.cash_reserve = debt_side
                .reserves
                .cash_reserve
                .checked_add(self.repay_credit)
                .and_then(|value| value.checked_add(self.insurance_credit))
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

        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;
        Ok(LiquidationReceipt {
            repaid_amount: self.repay_credit,
            collateral_seized,
            collateral_to_liquidator,
            insurance_funded,
            insurance_drawn: self.insurance_credit,
            socialized_loss,
            remaining_debt: position_debt(market, margin_position, self.debt_asset)?,
        })
    }
}

pub fn insurance_request_for_liquidation(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset: MarketAsset,
    repay_credit: u64,
    max_insurance_draw: u64,
) -> Result<u64> {
    let debt_before = position_debt(market, margin_position, debt_asset)?;
    require_gte!(
        debt_before,
        repay_credit as u128,
        ErrorCode::InsufficientDebt
    );
    let collateral_before = position_collateral(margin_position, debt_asset);
    let collateral_seized =
        collateral_to_seize(market, debt_asset, repay_credit, collateral_before)?;
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
    Ok(remaining_debt_cap.min(available).min(max_insurance_draw))
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
) -> Result<u64> {
    let seizure = market.collateral_amount_for_debt_value(debt_asset, repay_credit)?;
    Ok(seizure.min(collateral_before))
}

fn collateral_to_liquidator(
    market: &Market,
    debt_asset: MarketAsset,
    repay_credit: u64,
    collateral_seized: u64,
) -> Result<u64> {
    let liquidator_collateral =
        market.collateral_amount_for_liquidator_debt_value(debt_asset, repay_credit)?;
    Ok(liquidator_collateral.min(collateral_seized))
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
