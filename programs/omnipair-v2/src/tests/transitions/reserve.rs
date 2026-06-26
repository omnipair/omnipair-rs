use super::*;
    use crate::state::MarketSide;

    #[test]
    fn add_liquidity_mints_floating_ylp_shares() {
        let mut base_side = MarketSide::default();
        let mut quote_side = MarketSide::default();

        let receipt = AddLiquidity::new(1_000_000, 2_000_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        assert_eq!(receipt.base_ylp_amount, 1_000_000);
        assert_eq!(receipt.quote_ylp_amount, 2_000_000);
        assert_eq!(base_side.shares.ylp_supply, 1_000_000);
        assert_eq!(quote_side.shares.ylp_supply, 2_000_000);
    }

    #[test]
    fn remove_liquidity_burns_matched_proportions() {
        let mut base_side = MarketSide::default();
        let mut quote_side = MarketSide::default();
        AddLiquidity::new(1_000_000, 2_000_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        let receipt = RemoveLiquidity::new(250_000, 500_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        assert_eq!(receipt.base_amount_out, 250_000);
        assert_eq!(receipt.quote_amount_out, 500_000);
        assert_eq!(receipt.base_ylp_supply, 750_000);
        assert_eq!(receipt.quote_ylp_supply, 1_500_000);
    }
