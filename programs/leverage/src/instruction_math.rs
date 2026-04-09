//! Pure numeric helpers shared by instruction handlers; unit-tested without RPC or CPI.

use anchor_lang::prelude::*;
use omnipair::ceil_div;

use crate::{constants::*, errors::LeverageError};

/// Amounts derived for an open (multiply) instruction before CPI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MultiplyAmounts {
    pub swap_amount_in: u64,
    pub borrow_amount: u64,
    pub flashloan_fee: u64,
    pub repay_amount: u64,
    pub min_amount_out: u64,
}

/// Same formulas as `instructions::multiply::handle` (spot floor + flashloan fee on borrow only).
pub(crate) fn compute_multiply_amounts(
    lev_collateral_amount: u64,
    multiplier_bps: u64,
    max_slippage_bps: u64,
    reserve_in: u64,
    reserve_out: u64,
) -> Result<MultiplyAmounts> {
    require!(lev_collateral_amount > 0, LeverageError::AmountZero);
    require!(multiplier_bps > BPS_DENOMINATOR, LeverageError::MultiplierTooLow);
    require!(max_slippage_bps <= BPS_DENOMINATOR, LeverageError::InvalidSlippage);
    require!(reserve_in > 0 && reserve_out > 0, LeverageError::InsufficientLiquidity);

    let swap_amount_in: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(LeverageError::Overflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(LeverageError::Overflow)?
        .try_into()
        .map_err(|_| LeverageError::Overflow)?;

    let borrow_amount = swap_amount_in
        .checked_sub(lev_collateral_amount)
        .ok_or(LeverageError::Overflow)?;

    let flashloan_fee = ceil_div(
        (borrow_amount as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(LeverageError::Overflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(LeverageError::Overflow)? as u64;

    let repay_amount = borrow_amount
        .checked_add(flashloan_fee)
        .ok_or(LeverageError::Overflow)?;

    let spot_out: u64 = (lev_collateral_amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(LeverageError::Overflow)?
        .checked_mul(reserve_out as u128)
        .ok_or(LeverageError::Overflow)?
        .checked_div(
            (reserve_in as u128)
                .checked_mul(BPS_DENOMINATOR as u128)
                .ok_or(LeverageError::Overflow)?,
        )
        .ok_or(LeverageError::Overflow)?
        .try_into()
        .map_err(|_| LeverageError::Overflow)?;

    let min_amount_out: u64 = ((spot_out as u128)
        .saturating_mul((BPS_DENOMINATOR as u128).saturating_sub(max_slippage_bps as u128))
        / BPS_DENOMINATOR as u128)
        .try_into()
        .map_err(|_| LeverageError::Overflow)?;

    Ok(MultiplyAmounts {
        swap_amount_in,
        borrow_amount,
        flashloan_fee,
        repay_amount,
        min_amount_out,
    })
}

/// Flashloan fee and total repay for close path (`close_multiply`), same as `close_multiply::handle`.
pub(crate) fn compute_close_repay_amounts(debt_amount: u64) -> Result<(u64, u64)> {
    require!(debt_amount > 0, LeverageError::PositionNotOpen);

    let flashloan_fee = ceil_div(
        (debt_amount as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(LeverageError::Overflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(LeverageError::Overflow)? as u64;

    let repay_amount = debt_amount
        .checked_add(flashloan_fee)
        .ok_or(LeverageError::Overflow)?;

    Ok((flashloan_fee, repay_amount))
}

/// `token0_in` for the callback swap direction: `is_lev_collateral0 XOR is_close`.
#[inline]
pub(crate) fn callback_swap_token0_is_input(is_lev_collateral0: bool, is_close: bool) -> bool {
    is_lev_collateral0 ^ is_close
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiply_2x_borrow_and_fee() {
        let m = compute_multiply_amounts(1_000, 20_000, 0, 1_000_000, 1_000_000).unwrap();
        assert_eq!(m.swap_amount_in, 2_000);
        assert_eq!(m.borrow_amount, 1_000);
        assert_eq!(m.flashloan_fee, 1);
        assert_eq!(m.repay_amount, 1_001);
        assert_eq!(m.min_amount_out, 2_000);
    }

    #[test]
    fn multiply_symmetric_reserves_spot_matches_constant_product_intuition() {
        let m = compute_multiply_amounts(100, 15_000, 0, 10_000, 10_000).unwrap();
        assert_eq!(m.swap_amount_in, 150);
        assert_eq!(m.borrow_amount, 50);
        let spot_out = (100u128 * 15_000 * 10_000) / (10_000 * 10_000);
        assert_eq!(spot_out, 150);
        assert_eq!(m.min_amount_out as u128, spot_out);
    }

    #[test]
    fn multiply_slippage_clamps_min_out() {
        let no_slip = compute_multiply_amounts(100, 20_000, 0, 1_000, 2_000).unwrap();
        let slip_1pct = compute_multiply_amounts(100, 20_000, 100, 1_000, 2_000).unwrap();
        assert_eq!(slip_1pct.min_amount_out, no_slip.min_amount_out * 9_900 / 10_000);
    }

    #[test]
    fn multiply_rejects_zero_collateral() {
        assert!(compute_multiply_amounts(0, 20_000, 0, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_multiplier_at_1x() {
        assert!(compute_multiply_amounts(100, 10_000, 0, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_slippage_over_100pct() {
        assert!(compute_multiply_amounts(100, 20_000, 10_001, 1, 1).is_err());
    }

    #[test]
    fn multiply_rejects_empty_reserves() {
        assert!(compute_multiply_amounts(100, 20_000, 0, 0, 1).is_err());
    }

    #[test]
    fn close_repay_rounds_fee_up() {
        let (fee, repay) = compute_close_repay_amounts(1).unwrap();
        assert_eq!(fee, 1);
        assert_eq!(repay, 2);
    }

    #[test]
    fn close_repay_zero_debt_errors() {
        assert!(compute_close_repay_amounts(0).is_err());
    }

    #[test]
    fn callback_token0_in_truth_table() {
        // Matches flash_loan_callback comments: open+long0 → token0 in; close+long0 → token1 in; etc.
        assert!(callback_swap_token0_is_input(true, false));
        assert!(!callback_swap_token0_is_input(false, false));
        assert!(!callback_swap_token0_is_input(true, true));
        assert!(callback_swap_token0_is_input(false, true));
    }
}
