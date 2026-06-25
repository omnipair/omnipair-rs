use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{Debt, MarginPosition, Market, MarketAsset, MarketSide},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebtReceipt {
    pub debt_delta: i64,
    pub fixed_base_debt: u128,
    pub fixed_quote_debt: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
}

pub struct Borrow {
    pub borrow_asset: MarketAsset,
    pub borrow_amount: u64,
    pub min_health_bps: u64,
}

pub struct Repay {
    pub repay_asset: MarketAsset,
    pub repay_credit: u64,
}

impl DebtReceipt {
    fn from_market(market: &Market, debt_delta: i64) -> Result<Self> {
        Ok(Self {
            debt_delta,
            fixed_base_debt: market.debt.fixed_base_debt()?,
            fixed_quote_debt: market.debt.fixed_quote_debt()?,
            base_debt_health_bps: market.health.base_debt_health_bps,
            quote_debt_health_bps: market.health.quote_debt_health_bps,
        })
    }
}

impl Borrow {
    pub fn new(borrow_asset: MarketAsset, borrow_amount: u64, min_health_bps: u64) -> Self {
        Self {
            borrow_asset,
            borrow_amount,
            min_health_bps,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<DebtReceipt> {
        let debt_delta = i64::try_from(self.borrow_amount).map_err(|_| ErrorCode::Overflow)?;
        let debt_shares = match self.borrow_asset {
            MarketAsset::Base => {
                Debt::debt_to_shares(self.borrow_amount, market.debt.base_borrow_index_nad)?
            }
            MarketAsset::Quote => {
                Debt::debt_to_shares(self.borrow_amount, market.debt.quote_borrow_index_nad)?
            }
        };
        market.enforce_daily_borrow_limit(self.borrow_asset, self.borrow_amount)?;
        let debt_side = market.side_mut(self.borrow_asset)?;
        require_borrow_headroom(debt_side, self.borrow_amount)?;
        debt_side.reserves.live_reserve = debt_side
            .reserves
            .live_reserve
            .checked_sub(self.borrow_amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        debt_side.reserves.cash_reserve = debt_side
            .reserves
            .cash_reserve
            .checked_sub(self.borrow_amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        match self.borrow_asset {
            MarketAsset::Base => {
                margin_position.fixed_base_shares = margin_position
                    .fixed_base_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.fixed_base_shares = market
                    .debt
                    .fixed_base_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                margin_position.fixed_quote_shares = margin_position
                    .fixed_quote_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.fixed_quote_shares = market
                    .debt
                    .fixed_quote_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        sync_borrow_recognition(market, margin_position, self.borrow_asset)?;
        market.refresh_market_health()?;
        market.assert_market_health()?;
        market.assert_risk_circuit_breakers()?;
        market.assert_recognition_cap(margin_position, self.borrow_asset)?;
        market.assert_position_health(margin_position, self.borrow_asset, self.min_health_bps)?;
        let health = market.position_health_bps(margin_position, self.borrow_asset)?;
        require_gte!(
            health,
            self.min_health_bps,
            ErrorCode::InsufficientMarketHealth
        );
        DebtReceipt::from_market(market, debt_delta)
    }
}

impl Repay {
    pub fn new(repay_asset: MarketAsset, repay_credit: u64) -> Self {
        Self {
            repay_asset,
            repay_credit,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<DebtReceipt> {
        let debt_delta = -i64::try_from(self.repay_credit).map_err(|_| ErrorCode::Overflow)?;
        match self.repay_asset {
            MarketAsset::Base => {
                let debt_before = margin_position.fixed_base_debt(&market.debt)?;
                require_gte!(
                    debt_before,
                    self.repay_credit as u128,
                    ErrorCode::InsufficientDebt
                );
                let shares_before = margin_position.fixed_base_shares;
                let shares_to_burn = if self.repay_credit as u128 == debt_before {
                    shares_before
                } else {
                    Debt::debt_to_shares(self.repay_credit, market.debt.base_borrow_index_nad)?
                        .min(shares_before)
                };
                let release_collateral = proportional_release(
                    margin_position.recognized_quote_collateral_for_base_debt,
                    shares_to_burn,
                    shares_before,
                )?;
                margin_position.fixed_base_shares = margin_position
                    .fixed_base_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                margin_position.recognized_quote_collateral_for_base_debt = margin_position
                    .recognized_quote_collateral_for_base_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.fixed_base_shares = market
                    .debt
                    .fixed_base_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.recognized_quote_collateral_for_base_debt = market
                    .debt
                    .recognized_quote_collateral_for_base_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.base_side.reserves.live_reserve = market
                    .base_side
                    .reserves
                    .live_reserve
                    .checked_add(self.repay_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                market.base_side.reserves.cash_reserve = market
                    .base_side
                    .reserves
                    .cash_reserve
                    .checked_add(self.repay_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
            }
            MarketAsset::Quote => {
                let debt_before = margin_position.fixed_quote_debt(&market.debt)?;
                require_gte!(
                    debt_before,
                    self.repay_credit as u128,
                    ErrorCode::InsufficientDebt
                );
                let shares_before = margin_position.fixed_quote_shares;
                let shares_to_burn = if self.repay_credit as u128 == debt_before {
                    shares_before
                } else {
                    Debt::debt_to_shares(self.repay_credit, market.debt.quote_borrow_index_nad)?
                        .min(shares_before)
                };
                let release_collateral = proportional_release(
                    margin_position.recognized_base_collateral_for_quote_debt,
                    shares_to_burn,
                    shares_before,
                )?;
                margin_position.fixed_quote_shares = margin_position
                    .fixed_quote_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                margin_position.recognized_base_collateral_for_quote_debt = margin_position
                    .recognized_base_collateral_for_quote_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.fixed_quote_shares = market
                    .debt
                    .fixed_quote_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.debt.recognized_base_collateral_for_quote_debt = market
                    .debt
                    .recognized_base_collateral_for_quote_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                market.quote_side.reserves.live_reserve = market
                    .quote_side
                    .reserves
                    .live_reserve
                    .checked_add(self.repay_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                market.quote_side.reserves.cash_reserve = market
                    .quote_side
                    .reserves
                    .cash_reserve
                    .checked_add(self.repay_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
            }
        }
        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;
        DebtReceipt::from_market(market, debt_delta)
    }
}

fn sync_borrow_recognition(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset: MarketAsset,
) -> Result<()> {
    let risk = market.current_risk()?;
    let recognition_slot = Clock::get()
        .map(|clock| clock.slot)
        .unwrap_or(market.last_update_slot);

    match debt_asset {
        MarketAsset::Base => {
            let old_recognized = margin_position.recognized_quote_collateral_for_base_debt;
            let target_recognized =
                market.debt_capped_recognized_collateral(margin_position, debt_asset, &risk)?;
            reconcile_recognition(
                &mut margin_position.recognized_quote_collateral_for_base_debt,
                &mut market.debt.recognized_quote_collateral_for_base_debt,
                old_recognized,
                target_recognized,
            )?;
        }
        MarketAsset::Quote => {
            let old_recognized = margin_position.recognized_base_collateral_for_quote_debt;
            let target_recognized =
                market.debt_capped_recognized_collateral(margin_position, debt_asset, &risk)?;
            reconcile_recognition(
                &mut margin_position.recognized_base_collateral_for_quote_debt,
                &mut market.debt.recognized_base_collateral_for_quote_debt,
                old_recognized,
                target_recognized,
            )?;
        }
    }

    market.debt.last_recognition_slot = recognition_slot;
    Ok(())
}

fn reconcile_recognition(
    position_recognized: &mut u64,
    market_recognized: &mut u64,
    old_recognized: u64,
    target_recognized: u64,
) -> Result<()> {
    match target_recognized.cmp(&old_recognized) {
        std::cmp::Ordering::Greater => {
            let delta = target_recognized
                .checked_sub(old_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *market_recognized = market_recognized
                .checked_add(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Less => {
            let delta = old_recognized
                .checked_sub(target_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *market_recognized = market_recognized
                .checked_sub(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Equal => {}
    }

    *position_recognized = target_recognized;
    Ok(())
}

fn require_borrow_headroom(debt_side: &MarketSide, borrow_amount: u64) -> Result<()> {
    require_gte!(
        debt_side.reserves.cash_reserve,
        borrow_amount,
        ErrorCode::InsufficientBorrowHeadroom
    );
    Ok(())
}

fn proportional_release(recognized: u64, shares_to_burn: u128, shares_before: u128) -> Result<u64> {
    require!(shares_before > 0, ErrorCode::InsufficientDebt);
    if shares_to_burn == shares_before {
        return Ok(recognized);
    }
    let release = (recognized as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(release).map_err(|_| ErrorCode::MarketMathOverflow.into())
}
