//! Pure numeric helpers shared by instruction handlers; unit-tested without RPC or CPI.
//!
//! The source of truth lives in `omnipair` because native leverage execution now
//! happens there. The leverage wrapper keeps these thin shims for local tests and
//! to avoid duplicating formulas.

use anchor_lang::prelude::*;

pub(crate) use omnipair::MultiplyAmounts;

pub(crate) fn compute_multiply_amounts(
    lev_collateral_amount: u64,
    multiplier_bps: u64,
    max_slippage_bps: u64,
    reserve_in: u64,
    reserve_out: u64,
) -> Result<MultiplyAmounts> {
    omnipair::compute_multiply_amounts(
        lev_collateral_amount,
        multiplier_bps,
        max_slippage_bps,
        reserve_in,
        reserve_out,
    )
}

pub(crate) fn compute_close_repay_amounts(debt_amount: u64) -> Result<(u64, u64)> {
    omnipair::compute_close_repay_amounts(debt_amount)
}

#[inline]
pub(crate) fn leverage_swap_token0_is_input(is_lev_collateral0: bool, is_close: bool) -> bool {
    omnipair::leverage_swap_token0_is_input(is_lev_collateral0, is_close)
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
        assert_eq!(
            slip_1pct.min_amount_out,
            no_slip.min_amount_out * 9_900 / 10_000
        );
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
    fn leverage_token0_in_truth_table() {
        // Open+long0 -> token0 in; close+long0 -> token1 in; etc.
        assert!(leverage_swap_token0_is_input(true, false));
        assert!(!leverage_swap_token0_is_input(false, false));
        assert!(!leverage_swap_token0_is_input(true, true));
        assert!(leverage_swap_token0_is_input(false, true));
    }
}
