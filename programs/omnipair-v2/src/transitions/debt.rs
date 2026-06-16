use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{DebtBook, MarginPosition, Market, MarketSide},
    utils::market_math::require_market_reserve_floor,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebtReceipt {
    pub debt_delta: i64,
    pub fixed_debt0: u128,
    pub fixed_debt1: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
}

pub struct Borrow {
    pub borrow_asset_is_asset0: bool,
    pub borrow_amount: u64,
    pub min_health_bps: u64,
}

pub struct Repay {
    pub repay_asset_is_asset0: bool,
    pub repay_credit: u64,
}

impl DebtReceipt {
    fn from_market(market: &Market, debt_delta: i64) -> Result<Self> {
        Ok(Self {
            debt_delta,
            fixed_debt0: market.debt_book.fixed_debt0()?,
            fixed_debt1: market.debt_book.fixed_debt1()?,
            health0_bps: market.health.health0_bps,
            health1_bps: market.health.health1_bps,
        })
    }
}

impl Borrow {
    pub fn new(borrow_asset_is_asset0: bool, borrow_amount: u64, min_health_bps: u64) -> Self {
        Self {
            borrow_asset_is_asset0,
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
        let debt_shares = if self.borrow_asset_is_asset0 {
            DebtBook::debt_to_shares(self.borrow_amount, market.debt_book.borrow_index0_nad)?
        } else {
            DebtBook::debt_to_shares(self.borrow_amount, market.debt_book.borrow_index1_nad)?
        };
        let debt_side_index = if self.borrow_asset_is_asset0 { 0 } else { 1 };
        market.enforce_daily_borrow_limit(debt_side_index, self.borrow_amount)?;
        let debt_side = if self.borrow_asset_is_asset0 {
            &mut market.base_side
        } else {
            &mut market.quote_side
        };
        require_borrow_headroom(debt_side, self.borrow_amount)?;
        debt_side.reserve_ledger.live_reserve = debt_side
            .reserve_ledger
            .live_reserve
            .checked_sub(self.borrow_amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        debt_side.reserve_ledger.cash_reserve = debt_side
            .reserve_ledger
            .cash_reserve
            .checked_sub(self.borrow_amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        if self.borrow_asset_is_asset0 {
            margin_position.fixed_debt0_shares = margin_position
                .fixed_debt0_shares
                .checked_add(debt_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt_book.fixed_debt0_shares = market
                .debt_book
                .fixed_debt0_shares
                .checked_add(debt_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        } else {
            margin_position.fixed_debt1_shares = margin_position
                .fixed_debt1_shares
                .checked_add(debt_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt_book.fixed_debt1_shares = market
                .debt_book
                .fixed_debt1_shares
                .checked_add(debt_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        sync_borrow_recognition(market, margin_position, self.borrow_asset_is_asset0)?;
        market.refresh_market_health()?;
        market.assert_market_health()?;
        market.assert_risk_circuit_breakers()?;
        market.assert_recognition_cap(margin_position, self.borrow_asset_is_asset0)?;
        market.assert_position_health(
            margin_position,
            self.borrow_asset_is_asset0,
            self.min_health_bps,
        )?;
        let health = if self.borrow_asset_is_asset0 {
            market.position_health_bps(margin_position, true)?
        } else {
            market.position_health_bps(margin_position, false)?
        };
        require_gte!(
            health,
            self.min_health_bps,
            ErrorCode::InsufficientMarketHealth
        );
        DebtReceipt::from_market(market, debt_delta)
    }
}

impl Repay {
    pub fn new(repay_asset_is_asset0: bool, repay_credit: u64) -> Self {
        Self {
            repay_asset_is_asset0,
            repay_credit,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<DebtReceipt> {
        let debt_delta = -i64::try_from(self.repay_credit).map_err(|_| ErrorCode::Overflow)?;
        if self.repay_asset_is_asset0 {
            let debt_before = margin_position.fixed_debt0(&market.debt_book)?;
            require_gte!(
                debt_before,
                self.repay_credit as u128,
                ErrorCode::InsufficientDebt
            );
            let shares_before = margin_position.fixed_debt0_shares;
            let shares_to_burn = if self.repay_credit as u128 == debt_before {
                shares_before
            } else {
                DebtBook::debt_to_shares(self.repay_credit, market.debt_book.borrow_index0_nad)?
                    .min(shares_before)
            };
            let release_collateral = proportional_release(
                margin_position.recognized_collateral1_for_debt0,
                shares_to_burn,
                shares_before,
            )?;
            margin_position.fixed_debt0_shares = margin_position
                .fixed_debt0_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            margin_position.recognized_collateral1_for_debt0 = margin_position
                .recognized_collateral1_for_debt0
                .checked_sub(release_collateral)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt_book.fixed_debt0_shares = market
                .debt_book
                .fixed_debt0_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.recognition_ledger.debt_bearing_collateral1_for_debt0 = market
                .recognition_ledger
                .debt_bearing_collateral1_for_debt0
                .checked_sub(release_collateral)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.base_side.reserve_ledger.live_reserve = market
                .base_side
                .reserve_ledger
                .live_reserve
                .checked_add(self.repay_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
            market.base_side.reserve_ledger.cash_reserve = market
                .base_side
                .reserve_ledger
                .cash_reserve
                .checked_add(self.repay_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
        } else {
            let debt_before = margin_position.fixed_debt1(&market.debt_book)?;
            require_gte!(
                debt_before,
                self.repay_credit as u128,
                ErrorCode::InsufficientDebt
            );
            let shares_before = margin_position.fixed_debt1_shares;
            let shares_to_burn = if self.repay_credit as u128 == debt_before {
                shares_before
            } else {
                DebtBook::debt_to_shares(self.repay_credit, market.debt_book.borrow_index1_nad)?
                    .min(shares_before)
            };
            let release_collateral = proportional_release(
                margin_position.recognized_collateral0_for_debt1,
                shares_to_burn,
                shares_before,
            )?;
            margin_position.fixed_debt1_shares = margin_position
                .fixed_debt1_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            margin_position.recognized_collateral0_for_debt1 = margin_position
                .recognized_collateral0_for_debt1
                .checked_sub(release_collateral)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.debt_book.fixed_debt1_shares = market
                .debt_book
                .fixed_debt1_shares
                .checked_sub(shares_to_burn)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.recognition_ledger.debt_bearing_collateral0_for_debt1 = market
                .recognition_ledger
                .debt_bearing_collateral0_for_debt1
                .checked_sub(release_collateral)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market.quote_side.reserve_ledger.live_reserve = market
                .quote_side
                .reserve_ledger
                .live_reserve
                .checked_add(self.repay_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
            market.quote_side.reserve_ledger.cash_reserve = market
                .quote_side
                .reserve_ledger
                .cash_reserve
                .checked_add(self.repay_credit)
                .ok_or(ErrorCode::ReserveOverflow)?;
        }
        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;
        DebtReceipt::from_market(market, debt_delta)
    }
}

fn sync_borrow_recognition(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset_is_asset0: bool,
) -> Result<()> {
    let risk_book = market.current_risk_book()?;
    let recognition_slot = Clock::get()
        .map(|clock| clock.slot)
        .unwrap_or(market.last_update_slot);

    if debt_asset_is_asset0 {
        let old_recognized = margin_position.recognized_collateral1_for_debt0;
        let target_recognized =
            market.debt_capped_recognized_collateral(margin_position, true, &risk_book)?;
        reconcile_recognition(
            &mut margin_position.recognized_collateral1_for_debt0,
            &mut market.recognition_ledger.debt_bearing_collateral1_for_debt0,
            old_recognized,
            target_recognized,
        )?;
    } else {
        let old_recognized = margin_position.recognized_collateral0_for_debt1;
        let target_recognized =
            market.debt_capped_recognized_collateral(margin_position, false, &risk_book)?;
        reconcile_recognition(
            &mut margin_position.recognized_collateral0_for_debt1,
            &mut market.recognition_ledger.debt_bearing_collateral0_for_debt1,
            old_recognized,
            target_recognized,
        )?;
    }

    market.recognition_ledger.last_recognition_slot = recognition_slot;
    Ok(())
}

fn reconcile_recognition(
    position_recognized: &mut u64,
    ledger_recognized: &mut u64,
    old_recognized: u64,
    target_recognized: u64,
) -> Result<()> {
    match target_recognized.cmp(&old_recognized) {
        std::cmp::Ordering::Greater => {
            let delta = target_recognized
                .checked_sub(old_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *ledger_recognized = ledger_recognized
                .checked_add(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Less => {
            let delta = old_recognized
                .checked_sub(target_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *ledger_recognized = ledger_recognized
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
        debt_side.reserve_ledger.cash_reserve,
        borrow_amount,
        ErrorCode::InsufficientBorrowHeadroom
    );
    let next_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    require_market_reserve_floor(
        next_reserve,
        debt_side.claim_token_ledger.protected_claim_token_supply,
        debt_side.buffer_ledger.required_buffer,
    )
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
