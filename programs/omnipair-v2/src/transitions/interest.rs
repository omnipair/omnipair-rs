//! Borrow-index accrual transition.
//!
//! Advances both per-asset borrow indices forward to the current slot using the
//! kinked utilization rate model. Utilization is measured over *all* borrowing
//! against a side — margin debt and the opposite-direction hLP vault's debt leg
//! — since both are denominated in that side's asset and share its index.
//!
//! Advancing the index is what charges interest: outstanding debt is valued as
//! `shares * index`, so borrowers owe more over time. The matching credit is
//! realized when that debt is repaid back into the reserve.

use anchor_lang::prelude::*;

use crate::{
    constants::TARGET_MS_PER_SLOT,
    errors::ErrorCode,
    math::{accrued_index_nad, utilization_bps},
    state::{Debt, Market, MarketAsset},
};

pub struct AccrueInterest {
    pub current_slot: u64,
}

impl AccrueInterest {
    pub fn new(current_slot: u64) -> Self {
        Self { current_slot }
    }

    pub fn apply(self, market: &mut Market) -> Result<()> {
        let last = market.debt.last_accrual_slot;
        // No forward time elapsed (or the clock moved backwards): nothing to do.
        if self.current_slot <= last {
            return Ok(());
        }
        let dt_ms = self
            .current_slot
            .checked_sub(last)
            .ok_or(ErrorCode::MarketMathOverflow)?
            .saturating_mul(TARGET_MS_PER_SLOT);

        let base_index = market.debt.base_borrow_index_nad;
        let quote_index = market.debt.quote_borrow_index_nad;

        let borrowed_base = total_borrowed(market, MarketAsset::Base, base_index)?;
        let borrowed_quote = total_borrowed(market, MarketAsset::Quote, quote_index)?;
        let base_util =
            utilization_bps(borrowed_base, market.base_side.reserves.cash_reserve as u128)?;
        let quote_util =
            utilization_bps(borrowed_quote, market.quote_side.reserves.cash_reserve as u128)?;

        market.debt.base_borrow_index_nad = accrued_index_nad(base_index, base_util, dt_ms)?;
        market.debt.quote_borrow_index_nad = accrued_index_nad(quote_index, quote_util, dt_ms)?;
        market.debt.last_accrual_slot = self.current_slot;
        Ok(())
    }
}

/// Total outstanding debt denominated in `asset` (margin fixed + soft debt plus
/// the opposite-direction hLP vault's borrowed leg), valued at `index_nad`.
fn total_borrowed(market: &Market, asset: MarketAsset, index_nad: u128) -> Result<u128> {
    let (margin_fixed, margin_soft, hlp_shares) = match asset {
        // Base-denominated debt: margin base legs + the quote-hLP's base borrow.
        MarketAsset::Base => (
            market.debt.fixed_base_shares,
            market.debt.soft_base_shares,
            market.quote_hlp_vault.debt_shares,
        ),
        // Quote-denominated debt: margin quote legs + the base-hLP's quote borrow.
        MarketAsset::Quote => (
            market.debt.fixed_quote_shares,
            market.debt.soft_quote_shares,
            market.base_hlp_vault.debt_shares,
        ),
    };
    let total_shares = margin_fixed
        .checked_add(margin_soft)
        .and_then(|value| value.checked_add(hlp_shares))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Debt::shares_to_debt(total_shares, index_nad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{MS_PER_YEAR, NAD, TARGET_MS_PER_SLOT},
        state::{
            Debt, HlpVault, Insurance, MarketConfig, MarketHealth, MarketSide, Reserves, Risk,
        },
    };

    fn slots_for_ms(ms: u64) -> u64 {
        ms / TARGET_MS_PER_SLOT
    }

    fn test_market(base_cash: u64, quote_cash: u64) -> Market {
        let mut base_side = MarketSide::default();
        base_side.reserves = Reserves {
            live_reserve: base_cash,
            cash_reserve: base_cash,
            reserved_liability: 0,
        };
        let mut quote_side = MarketSide::default();
        quote_side.reserves = Reserves {
            live_reserve: quote_cash,
            cash_reserve: quote_cash,
            reserved_liability: 0,
        };
        Market {
            version: 2,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            manager: Pubkey::new_unique(),
            base_side,
            quote_side,
            config: MarketConfig::default(),
            debt: Debt {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                last_accrual_slot: 0,
                ..Debt::default()
            },
            base_hlp_vault: HlpVault::default(),
            quote_hlp_vault: HlpVault::default(),
            risk: Risk::default(),
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            params_hash: [0u8; 32],
            last_update_slot: 0,
            reduce_only: false,
            bump: 255,
        }
    }

    #[test]
    fn no_time_elapsed_is_a_noop() {
        let mut market = test_market(1_000, 1_000);
        market.debt.last_accrual_slot = 100;
        AccrueInterest::new(100).apply(&mut market).unwrap();
        assert_eq!(market.debt.base_borrow_index_nad, NAD as u128);
        assert_eq!(market.debt.quote_borrow_index_nad, NAD as u128);
        assert_eq!(market.debt.last_accrual_slot, 100);
    }

    #[test]
    fn idle_market_does_not_accrue() {
        // Cash present but zero debt -> 0% utilization -> base (0) rate.
        let mut market = test_market(1_000_000, 1_000_000);
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        assert_eq!(market.debt.base_borrow_index_nad, NAD as u128);
        assert_eq!(market.debt.quote_borrow_index_nad, NAD as u128);
    }

    #[test]
    fn quote_borrowing_accrues_quote_index_only() {
        // Quote side: 800 borrowed via base-hLP, 200 idle cash -> 80% util (kink),
        // 10% APR over a year -> quote index *1.10. Base side has no debt.
        let mut market = test_market(1_000_000, 200);
        market.base_hlp_vault.debt_shares = 800; // quote-denominated debt
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        assert_eq!(market.debt.base_borrow_index_nad, NAD as u128);
        assert_eq!(
            market.debt.quote_borrow_index_nad,
            (NAD as u128) * 110 / 100
        );
        assert_eq!(market.debt.last_accrual_slot, slots_for_ms(MS_PER_YEAR));
    }

    #[test]
    fn margin_and_hlp_debt_both_count_toward_utilization() {
        // Quote debt = 400 margin + 400 base-hLP = 800 borrowed, 200 cash -> 80%.
        let mut market = test_market(1_000_000, 200);
        market.debt.fixed_quote_shares = 400;
        market.base_hlp_vault.debt_shares = 400;
        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();
        assert_eq!(
            market.debt.quote_borrow_index_nad,
            (NAD as u128) * 110 / 100
        );
    }

    #[test]
    fn accrual_charges_interest_to_outstanding_debt() {
        // 800 quote borrowed via base-hLP, 200 idle -> 80% util -> 10% APR / yr.
        let mut market = test_market(1_000_000, 200);
        market.base_hlp_vault.debt_shares = 800;
        let debt_before = Debt::shares_to_debt(
            market.base_hlp_vault.debt_shares,
            market.debt.quote_borrow_index_nad,
        )
        .unwrap();

        AccrueInterest::new(slots_for_ms(MS_PER_YEAR))
            .apply(&mut market)
            .unwrap();

        let debt_after = Debt::shares_to_debt(
            market.base_hlp_vault.debt_shares,
            market.debt.quote_borrow_index_nad,
        )
        .unwrap();
        // The borrower's outstanding debt grew by the accrued interest (+10%).
        assert_eq!(debt_before, 800);
        assert_eq!(debt_after, 880);
    }
}
