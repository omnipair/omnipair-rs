use super::*;
use crate::constants::NAD;

    #[test]
    fn fee_accrual_uses_growth_delta() {
        let fees = accrue_fee_liability(1_000_000, 3 * NAD as u128, NAD as u128).unwrap();
        assert_eq!(fees, 2_000_000);
    }

    #[test]
    fn total_liability_includes_manager_fee_buckets() {
        let mut fees = Fees {
            swap_fee_vault_balance: 700,
            interest_vault_balance: 300,
            manager_swap_fee_liability: 400,
            manager_interest_fee_liability: 100,
            protocol_fee_liability: 250,
            buyback_fee_liability: 50,
            ..Fees::default()
        };

        assert_eq!(fees.total_liability().unwrap(), 800);
        fees.manager_swap_fee_liability = 0;
        fees.manager_interest_fee_liability = 0;
        assert_eq!(fees.total_liability().unwrap(), 300);
    }

    #[test]
    fn auction_liabilities_settle_by_lane() {
        let mut fees = Fees {
            swap_fee_vault_balance: 700,
            protocol_fee_liability: 500,
            buyback_fee_liability: 200,
            ..Fees::default()
        };

        fees.settle_protocol_auction_liability(ProtocolAuctionLane::Fee, 125)
            .unwrap();
        fees.settle_protocol_auction_liability(ProtocolAuctionLane::Buyback, 50)
            .unwrap();

        assert_eq!(fees.protocol_fee_liability, 375);
        assert_eq!(fees.buyback_fee_liability, 150);
        fees.assert_backed().unwrap();
    }
