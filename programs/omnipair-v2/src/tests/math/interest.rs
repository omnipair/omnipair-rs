use super::*;
    use crate::constants::{
        INTEREST_ADJUSTMENT_SPEED_PER_YEAR, INTEREST_CURVE_STEEPNESS_NAD,
        INTEREST_MAX_ADAPTATION_STEP_NAD, INTEREST_MAX_RATE_AT_TARGET_NAD,
        INTEREST_MIN_RATE_AT_TARGET_NAD, INTEREST_TARGET_UTILIZATION_BPS,
    };

    const TARGET: u64 = INTEREST_TARGET_UTILIZATION_BPS;
    const STEEP: u128 = INTEREST_CURVE_STEEPNESS_NAD;

    fn nad(x: u128) -> u128 {
        x * NAD as u128
    }

    #[test]
    fn utilization_is_ratio_of_borrowed_to_supplied() {
        assert_eq!(utilization_bps(600, 400).unwrap(), 6_000);
        assert_eq!(utilization_bps(1_000, 0).unwrap(), 10_000);
        assert_eq!(utilization_bps(0, 0).unwrap(), 0);
    }

    #[test]
    fn error_is_zero_at_target_and_signed_around_it() {
        assert_eq!(utilization_error_nad(TARGET, TARGET).unwrap(), 0);
        // full utilization -> +NAD
        assert_eq!(utilization_error_nad(10_000, TARGET).unwrap(), NAD as i128);
        // zero utilization -> -NAD
        assert_eq!(utilization_error_nad(0, TARGET).unwrap(), -(NAD as i128));
    }

    #[test]
    fn curve_multiplier_spans_reciprocal_to_steepness() {
        // error 0 -> 1.0
        assert_eq!(curve_multiplier_nad(0, STEEP).unwrap(), NAD as u128);
        // error +1 -> steepness (4x)
        assert_eq!(curve_multiplier_nad(NAD as i128, STEEP).unwrap(), STEEP);
        // error -1 -> 1/steepness (0.25x)
        assert_eq!(
            curve_multiplier_nad(-(NAD as i128), STEEP).unwrap(),
            nad(1) / 4
        );
    }

    #[test]
    fn instantaneous_rate_scales_anchor_by_curve() {
        let anchor = nad(10) / 100; // 10% APR
                                    // at target -> equals anchor
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, 0, STEEP).unwrap(),
            anchor
        );
        // at full util -> 4x anchor
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, NAD as i128, STEEP).unwrap(),
            anchor * 4
        );
        // at zero util -> anchor/4
        assert_eq!(
            instantaneous_rate_apr_nad(anchor, -(NAD as i128), STEEP).unwrap(),
            anchor / 4
        );
    }

    #[test]
    fn anchor_rises_above_target_and_falls_below() {
        let anchor = nad(10) / 100;
        let up = adapt_rate_at_target_nad(
            anchor,
            NAD as i128, // util at 100%
            MS_PER_YEAR / 52,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        let down = adapt_rate_at_target_nad(
            anchor,
            -(NAD as i128),
            MS_PER_YEAR / 52,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert!(up > anchor, "anchor should rise above target");
        assert!(down < anchor, "anchor should fall below target");
    }

    #[test]
    fn anchor_is_clamped_to_bounds() {
        // Already at max, sustained high util -> stays at max.
        let capped = adapt_rate_at_target_nad(
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            NAD as i128,
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(capped, INTEREST_MAX_RATE_AT_TARGET_NAD);
        // Already at min, sustained low util -> stays at min.
        let floored = adapt_rate_at_target_nad(
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            -(NAD as i128),
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(floored, INTEREST_MIN_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn anchor_does_not_move_at_target() {
        let anchor = nad(7) / 100;
        let same = adapt_rate_at_target_nad(
            anchor,
            0,
            MS_PER_YEAR,
            INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
            INTEREST_MIN_RATE_AT_TARGET_NAD,
            INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MAX_ADAPTATION_STEP_NAD,
        )
        .unwrap();
        assert_eq!(same, anchor);
    }

    #[test]
    fn index_grows_by_apr_over_one_year() {
        // 10% APR for a year -> index * 1.10.
        let index = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR).unwrap();
        assert_eq!(index, nad(110) / 100);
    }

    #[test]
    fn index_unchanged_with_no_time_or_zero_rate() {
        assert_eq!(accrued_index_nad(nad(1), nad(10) / 100, 0).unwrap(), nad(1));
        assert_eq!(accrued_index_nad(nad(1), 0, MS_PER_YEAR).unwrap(), nad(1));
    }

    #[test]
    fn index_elapsed_time_is_capped() {
        let capped = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR * 100).unwrap();
        let one_year = accrued_index_nad(nad(1), nad(10) / 100, MS_PER_YEAR).unwrap();
        assert_eq!(capped, one_year);
    }

    #[test]
    fn full_repay_splits_exactly_principal_and_interest() {
        assert_eq!(realized_interest_split(110, 110, 100).unwrap(), (100, 10));
    }

    #[test]
    fn partial_repay_splits_proportionally() {
        assert_eq!(realized_interest_split(55, 110, 100).unwrap(), (50, 5));
    }

    #[test]
    fn no_accrued_interest_routes_nothing() {
        assert_eq!(realized_interest_split(100, 100, 100).unwrap(), (100, 0));
        assert_eq!(realized_interest_split(40, 100, 100).unwrap(), (40, 0));
    }

    #[test]
    fn repay_above_debt_is_rejected() {
        assert_eq!(
            realized_interest_split(120, 110, 100).unwrap_err(),
            error!(ErrorCode::InsufficientDebt)
        );
    }

    #[test]
    fn principal_rounds_down_so_interest_is_never_underfunded() {
        assert_eq!(realized_interest_split(1, 3, 2).unwrap(), (0, 1));
    }
