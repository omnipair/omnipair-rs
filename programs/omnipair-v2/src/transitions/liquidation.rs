use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{DebtBook, MarginPosition, Market},
};

pub struct Liquidation {
    pub debt_asset_is_base: bool,
    pub repay_credit: u64,
    pub insurance_spent: u64,
    pub insurance_credit: u64,
    pub max_socialized_loss: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiquidationReceipt {
    pub repaid_amount: u64,
    pub collateral_seized: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
}

impl Liquidation {
    pub fn new(
        debt_asset_is_base: bool,
        repay_credit: u64,
        insurance_spent: u64,
        insurance_credit: u64,
        max_socialized_loss: u64,
    ) -> Self {
        Self {
            debt_asset_is_base,
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
        let debt_before = position_debt(market, margin_position, self.debt_asset_is_base)?;
        require_gte!(
            debt_before,
            self.repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let collateral_before = position_collateral(margin_position, self.debt_asset_is_base);
        let collateral_seized = collateral_to_seize(
            market,
            self.debt_asset_is_base,
            self.repay_credit,
            collateral_before,
        )?;
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
            require!(
                socialized_loss == 0,
                ErrorCode::InsufficientInsuranceReserve
            );
        }

        let debt_reduction = repay_plus_insurance
            .checked_add(socialized_loss as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        apply_liquidation_debt_reduction(
            market,
            margin_position,
            self.debt_asset_is_base,
            debt_reduction,
            collateral_seized,
        )?;

        let debt_side = if self.debt_asset_is_base {
            &mut market.base_side
        } else {
            &mut market.quote_side
        };
        debt_side.reserve_ledger.live_reserve = debt_side
            .reserve_ledger
            .live_reserve
            .checked_add(self.repay_credit)
            .and_then(|value| value.checked_add(self.insurance_credit))
            .ok_or(ErrorCode::ReserveOverflow)?;
        debt_side.reserve_ledger.cash_reserve = debt_side
            .reserve_ledger
            .cash_reserve
            .checked_add(self.repay_credit)
            .and_then(|value| value.checked_add(self.insurance_credit))
            .ok_or(ErrorCode::ReserveOverflow)?;
        if self.debt_asset_is_base {
            market.insurance_reserve.base_available = market
                .insurance_reserve
                .base_available
                .checked_sub(self.insurance_spent)
                .ok_or(ErrorCode::InsufficientInsuranceReserve)?;
        } else {
            market.insurance_reserve.quote_available = market
                .insurance_reserve
                .quote_available
                .checked_sub(self.insurance_spent)
                .ok_or(ErrorCode::InsufficientInsuranceReserve)?;
        }

        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;
        Ok(LiquidationReceipt {
            repaid_amount: self.repay_credit,
            collateral_seized,
            insurance_drawn: self.insurance_credit,
            socialized_loss,
            remaining_debt: position_debt(market, margin_position, self.debt_asset_is_base)?,
        })
    }
}

pub fn insurance_request_for_liquidation(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset_is_base: bool,
    repay_credit: u64,
    max_insurance_draw: u64,
) -> Result<u64> {
    let debt_before = position_debt(market, margin_position, debt_asset_is_base)?;
    require_gte!(
        debt_before,
        repay_credit as u128,
        ErrorCode::InsufficientDebt
    );
    let collateral_before = position_collateral(margin_position, debt_asset_is_base);
    let collateral_seized =
        collateral_to_seize(market, debt_asset_is_base, repay_credit, collateral_before)?;
    let remaining_debt = debt_before
        .checked_sub(repay_credit as u128)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if collateral_seized < collateral_before || remaining_debt == 0 {
        return Ok(0);
    }
    let available = if debt_asset_is_base {
        market.insurance_reserve.base_available
    } else {
        market.insurance_reserve.quote_available
    };
    let remaining_debt_cap = u64::try_from(remaining_debt).unwrap_or(u64::MAX);
    Ok(remaining_debt_cap.min(available).min(max_insurance_draw))
}

fn apply_liquidation_debt_reduction(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset_is_base: bool,
    debt_reduction: u128,
    collateral_seized: u64,
) -> Result<()> {
    if debt_asset_is_base {
        let shares_before = margin_position.fixed_base_debt_shares;
        let debt_before = margin_position.fixed_base_debt(&market.debt_book)?;
        let shares_to_burn = shares_to_burn_for_reduction(
            debt_reduction,
            debt_before,
            shares_before,
            market.debt_book.base_borrow_index_nad,
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
        margin_position.fixed_base_debt_shares = margin_position
            .fixed_base_debt_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_base_debt_shares = market
            .debt_book
            .fixed_base_debt_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflow)?;
    } else {
        let shares_before = margin_position.fixed_quote_debt_shares;
        let debt_before = margin_position.fixed_quote_debt(&market.debt_book)?;
        let shares_to_burn = shares_to_burn_for_reduction(
            debt_reduction,
            debt_before,
            shares_before,
            market.debt_book.quote_borrow_index_nad,
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
        margin_position.fixed_quote_debt_shares = margin_position
            .fixed_quote_debt_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_quote_debt_shares = market
            .debt_book
            .fixed_quote_debt_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market
            .recognition_ledger
            .debt_bearing_base_collateral_for_quote_debt = market
            .recognition_ledger
            .debt_bearing_base_collateral_for_quote_debt
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflow)?;
    }
    Ok(())
}

fn position_debt(
    market: &Market,
    margin_position: &MarginPosition,
    debt_asset_is_base: bool,
) -> Result<u128> {
    if debt_asset_is_base {
        margin_position.fixed_base_debt(&market.debt_book)
    } else {
        margin_position.fixed_quote_debt(&market.debt_book)
    }
}

fn position_collateral(margin_position: &MarginPosition, debt_asset_is_base: bool) -> u64 {
    if debt_asset_is_base {
        margin_position.quote_collateral
    } else {
        margin_position.base_collateral
    }
}

fn collateral_to_seize(
    market: &Market,
    debt_asset_is_base: bool,
    repay_credit: u64,
    collateral_before: u64,
) -> Result<u64> {
    let seizure = market.collateral_amount_for_debt_value(debt_asset_is_base, repay_credit)?;
    Ok(seizure.min(collateral_before))
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
    DebtBook::debt_to_shares(debt_reduction, borrow_index_nad)
        .map(|shares| shares.min(shares_before))
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
    use super::*;
    use crate::{
        constants::{MARKET_VERSION, NAD},
        state::{BufferLedger, MarketConfig, MarketSide, ReserveLedger},
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
                live_reserve: 1_000,
                cash_reserve: 1_000,
                reserved_liability: 0,
            },
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint),
            market_side(quote_mint),
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
            [9_u8; 32],
            42,
            253,
        )
        .unwrap();
        assert_eq!(market.version, MARKET_VERSION);
        market.insurance_reserve.base_available = 40;
        market.insurance_reserve.quote_available = 40;
        market
    }

    fn insolvent_position(market: &mut Market) -> MarginPosition {
        let debt_shares =
            DebtBook::debt_to_shares(100, market.debt_book.base_borrow_index_nad).unwrap();
        market.debt_book.fixed_base_debt_shares = debt_shares;
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 50;

        MarginPosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            base_collateral: 0,
            quote_collateral: 50,
            recognized_base_collateral_for_quote_debt: 0,
            recognized_quote_collateral_for_base_debt: 50,
            fixed_base_debt_shares: debt_shares,
            fixed_quote_debt_shares: 0,
            bump: 1,
        }
    }

    #[test]
    fn insurance_request_starts_after_collateral_is_exhausted() {
        let mut market = test_market();
        let position = insolvent_position(&mut market);

        let partial_request =
            insurance_request_for_liquidation(&market, &position, true, 25, 30).unwrap();
        assert_eq!(partial_request, 0);

        let exhausted_request =
            insurance_request_for_liquidation(&market, &position, true, 50, 30).unwrap();
        assert_eq!(exhausted_request, 30);
    }

    #[test]
    fn insurance_request_saturates_large_remaining_debt() {
        let mut market = test_market();
        let mut position = insolvent_position(&mut market);
        position.quote_collateral = 0;
        position.recognized_quote_collateral_for_base_debt = 0;
        position.fixed_base_debt_shares = (u64::MAX as u128) + 51;

        let request = insurance_request_for_liquidation(&market, &position, true, 50, 40).unwrap();

        assert_eq!(request, 40);
    }

    #[test]
    fn liquidation_uses_repay_insurance_then_socialization() {
        let mut market = test_market();
        let mut position = insolvent_position(&mut market);

        let receipt = Liquidation::new(true, 50, 30, 30, 20)
            .apply(&mut market, &mut position)
            .unwrap();

        assert_eq!(receipt.repaid_amount, 50);
        assert_eq!(receipt.collateral_seized, 50);
        assert_eq!(receipt.insurance_drawn, 30);
        assert_eq!(receipt.socialized_loss, 20);
        assert_eq!(receipt.remaining_debt, 0);
        assert_eq!(position.quote_collateral, 0);
        assert_eq!(position.recognized_quote_collateral_for_base_debt, 0);
        assert_eq!(position.fixed_base_debt_shares, 0);
        assert_eq!(market.debt_book.fixed_base_debt_shares, 0);
        assert_eq!(market.insurance_reserve.base_available, 10);
        assert_eq!(market.base_side.reserve_ledger.live_reserve, 1_080);
        assert_eq!(market.base_side.reserve_ledger.cash_reserve, 1_080);
    }

    #[test]
    fn liquidation_rejects_socialization_above_caller_cap() {
        let mut market = test_market();
        let mut position = insolvent_position(&mut market);

        let err = Liquidation::new(true, 50, 30, 30, 19)
            .apply(&mut market, &mut position)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::LiquidationSocializationExceeded));
    }

    #[test]
    fn recognized_decrease_never_exceeds_remaining_collateral() {
        let decrease = recognized_decrease_after_seizure(80, 25, 250, 1_000).unwrap();

        assert_eq!(decrease, 55);
        assert_eq!(80 - decrease, 25);
    }
}
