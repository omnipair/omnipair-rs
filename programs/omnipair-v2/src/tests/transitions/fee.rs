use super::*;

    #[test]
    fn swap_protocol_fee_splits_between_auction_lanes_at_accrual() {
        let mut side = MarketSide::default();
        let receipt = side
            .record_swap_fee_credit(
            10_000,
            1_000,
            2_000,
            ProtocolAuctionSplit {
                fee_auction_bps: 7_500,
                buyback_auction_bps: 2_500,
            },
        )
        .unwrap();

        assert_eq!(receipt.manager_swap_fee_liability, 1_000);
        assert_eq!(receipt.manager_interest_fee_liability, 0);
        assert_eq!(receipt.protocol_fee_liability, 1_500);
        assert_eq!(receipt.buyback_fee_liability, 500);
        assert_eq!(receipt.unallocated_swap_fee_liability, 7_000);
        assert_eq!(receipt.swap_fee_vault_balance, 10_000);
        side.fees.assert_backed().unwrap();
    }

    #[test]
    fn interest_protocol_fee_splits_between_auction_lanes_at_accrual() {
        let mut side = MarketSide::default();
        let receipt = side
            .record_interest_credit(
            10_000,
            500,
            1_000,
            ProtocolAuctionSplit {
                fee_auction_bps: 4_000,
                buyback_auction_bps: 6_000,
            },
        )
        .unwrap();

        assert_eq!(receipt.manager_swap_fee_liability, 0);
        assert_eq!(receipt.manager_interest_fee_liability, 500);
        assert_eq!(receipt.protocol_fee_liability, 400);
        assert_eq!(receipt.buyback_fee_liability, 600);
        assert_eq!(receipt.unallocated_interest_liability, 8_500);
        assert_eq!(receipt.interest_vault_balance, 10_000);
        side.fees.assert_backed().unwrap();
    }

    #[test]
    fn invalid_auction_split_is_rejected_before_liabilities_move() {
        let mut side = MarketSide::default();
        let err = side
            .record_swap_fee_credit(
            10_000,
            0,
            1_000,
            ProtocolAuctionSplit {
                fee_auction_bps: 7_000,
                buyback_auction_bps: 4_000,
            },
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidDistribution));
        assert_eq!(side.fees.swap_fee_vault_balance, 0);
        assert_eq!(side.fees.protocol_fee_liability, 0);
        assert_eq!(side.fees.buyback_fee_liability, 0);
    }
