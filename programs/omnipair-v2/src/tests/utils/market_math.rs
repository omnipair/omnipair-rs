use super::*;

    #[test]
    fn fee_accrual_uses_growth_delta() {
        let fees = accrue_fee_liability(1_000_000, 3 * NAD as u128, NAD as u128).unwrap();
        assert_eq!(fees, 2_000_000);
    }
