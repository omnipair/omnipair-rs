use super::*;

    #[test]
    fn pessimistic_reserve_value_uses_normalized_curve_output() {
        let value = collateral_value_from_pessimistic_reserves_nad(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();

        assert_eq!(value, 181_818_181);
    }

    #[test]
    fn debt_value_to_collateral_amount_uses_requested_rounding() {
        let ceil_amount = collateral_amount_for_debt_amount_ceil(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();
        let floor_amount = collateral_amount_for_debt_value_floor(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();

        assert_eq!(ceil_amount, 52_632);
        assert_eq!(floor_amount, 52_631);
    }

    #[test]
    fn daily_limit_from_liquidity_ema_sizes_by_bps_and_decimals() {
        let limit = daily_limit_from_liquidity_ema(1_000 * NAD as u128, 6, 2_000).unwrap();

        assert_eq!(limit, 200_000_000);
    }

    #[test]
    fn daily_limit_from_liquidity_ema_rejects_zero_liquidity() {
        let err = daily_limit_from_liquidity_ema(0, 6, 2_000).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientLiquidity));
    }
