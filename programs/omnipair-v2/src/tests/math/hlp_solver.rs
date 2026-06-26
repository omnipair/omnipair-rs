use super::*;

    fn nad(x: u128) -> u128 {
        x * NAD as u128
    }

    #[test]
    fn isqrt_matches_floor_sqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(8), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(1_000_000), 1_000);
        let big = (u64::MAX as u128) * (u64::MAX as u128);
        assert_eq!(isqrt(big), u64::MAX as u128);
    }

    #[test]
    fn sqrt_ratio_of_1_44_is_1_2() {
        // r = 1.44 -> sqrt = 1.2.
        let r = nad(144) / 100;
        assert_eq!(sqrt_ratio_nad(r).unwrap(), nad(12) / 10);
    }

    #[test]
    fn tracking_loss_matches_closed_form() {
        // E0 = 100, r = 1.44 -> loss = 100 * (1.2 - 1)^2 = 100 * 0.04 = 4.
        let loss = tracking_loss_nad(nad(100), nad(144) / 100).unwrap();
        assert_eq!(loss, nad(4));
    }

    #[test]
    fn tracking_loss_is_zero_below_unit_ratio() {
        assert_eq!(tracking_loss_nad(nad(100), nad(1)).unwrap(), 0);
        assert_eq!(tracking_loss_nad(nad(100), nad(8) / 10).unwrap(), 0);
    }

    #[test]
    fn closed_form_pre_adjustment_upside() {
        // E0 = 100, r = 1.44 -> Δpre = 100 * (1.2 - 1) = 20, lever up.
        let (amount, lever_up) = closed_form_pre_adjustment_nad(nad(100), nad(144) / 100).unwrap();
        assert_eq!(amount, nad(20));
        assert!(lever_up);
    }

    #[test]
    fn closed_form_pre_adjustment_downside_is_deleverage() {
        // r = 0.64 -> sqrt = 0.8 -> |Δpre| = 100 * 0.2 = 20, deleverage.
        let (amount, lever_up) = closed_form_pre_adjustment_nad(nad(100), nad(64) / 100).unwrap();
        assert_eq!(amount, nad(20));
        assert!(!lever_up);
    }

    #[test]
    fn bisect_finds_threshold_root() {
        // Residual f(x) = x - 1000 (root at 1000); smallest x with f(x) >= 0.
        let root = bisect(0, 1_000_000, 64, |x| Ok(x as i128 - 1_000)).unwrap();
        assert!(root >= 1_000 && root <= 1_001);
    }

    #[test]
    fn bisect_respects_iteration_budget() {
        // With only a few iterations it cannot fully converge on a wide range,
        // but it must stay within [lo, hi] and not panic.
        let root = bisect(0, u64::MAX as u128, 4, |x| Ok(x as i128 - 5)).unwrap();
        assert!(root <= u64::MAX as u128);
    }
