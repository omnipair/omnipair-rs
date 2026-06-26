use super::*;

    #[test]
    fn add_margin_principal_accumulates_per_side() {
        let mut debt = Debt::default();
        debt.add_margin_principal(MarketAsset::Base, 1_000).unwrap();
        debt.add_margin_principal(MarketAsset::Base, 500).unwrap();
        debt.add_margin_principal(MarketAsset::Quote, 200).unwrap();
        assert_eq!(debt.fixed_base_principal, 1_500);
        assert_eq!(debt.fixed_quote_principal, 200);
    }

    #[test]
    fn realize_margin_repay_is_all_principal_without_interest() {
        let mut debt = Debt {
            fixed_base_shares: 1_000,
            base_borrow_index_nad: NAD as u128,
            fixed_base_principal: 1_000,
            ..Debt::default()
        };
        let interest = debt.realize_margin_repay(MarketAsset::Base, 400).unwrap();
        assert_eq!(interest, 0);
        assert_eq!(debt.fixed_base_principal, 600);
    }

    #[test]
    fn realize_margin_repay_splits_accrued_interest() {
        // Index 1.1: 1_000 of principal now owes 1_100 of debt.
        let mut debt = Debt {
            fixed_base_shares: 1_000,
            base_borrow_index_nad: (NAD as u128) * 11 / 10,
            fixed_base_principal: 1_000,
            ..Debt::default()
        };
        // Repay 550 of 1_100: 500 principal + 50 interest.
        let interest = debt.realize_margin_repay(MarketAsset::Base, 550).unwrap();
        assert_eq!(interest, 50);
        assert_eq!(debt.fixed_base_principal, 500);
    }

    #[test]
    fn realize_margin_repay_full_clears_principal_and_returns_all_interest() {
        let mut debt = Debt {
            fixed_quote_shares: 1_000,
            quote_borrow_index_nad: (NAD as u128) * 11 / 10,
            fixed_quote_principal: 1_000,
            ..Debt::default()
        };
        let interest = debt
            .realize_margin_repay(MarketAsset::Quote, 1_100)
            .unwrap();
        assert_eq!(interest, 100);
        assert_eq!(debt.fixed_quote_principal, 0);
    }

    #[test]
    fn liquidation_writeoff_reduces_principal_without_realizing_interest_as_cash() {
        let mut debt = Debt {
            fixed_base_shares: 1_000,
            base_borrow_index_nad: (NAD as u128) * 11 / 10,
            fixed_base_principal: 1_000,
            ..Debt::default()
        };

        let interest = debt
            .realize_margin_liquidation(MarketAsset::Base, 550, 1_100)
            .unwrap();

        assert_eq!(interest, 50);
        assert_eq!(debt.fixed_base_principal, 0);
    }
