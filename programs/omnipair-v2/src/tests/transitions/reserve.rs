use super::*;

    fn empty_market() -> Market {
        Market {
            version: MARKET_VERSION,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            ylp_mint: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            manager: Pubkey::new_unique(),
            base_side: MarketSide::default(),
            quote_side: MarketSide::default(),
            config: MarketConfig::default(),
            debt: Debt::default(),
            base_hlp_vault: HlpVault::default(),
            quote_hlp_vault: HlpVault::default(),
            risk: Risk::default(),
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            pending_config: PendingConfigChange::default(),
            pending_operator: PendingAuthorityChange::default(),
            pending_manager: PendingAuthorityChange::default(),
            params_hash: [0u8; 32],
            last_update_slot: 0,
            reduce_only: false,
            bump: 255,
        }
    }

    #[test]
    fn add_liquidity_mints_floating_ylp_shares() {
        let mut market = empty_market();

        let receipt = market.add_liquidity(1_000_000, 2_000_000).unwrap();

        assert_eq!(receipt.ylp_amount, 1_000_000);
        assert_eq!(market.base_side.shares.ylp_supply, 1_000_000);
        assert_eq!(market.quote_side.shares.ylp_supply, 1_000_000);
    }

    #[test]
    fn remove_liquidity_burns_matched_proportions() {
        let mut market = empty_market();
        market.add_liquidity(1_000_000, 2_000_000).unwrap();

        let receipt = market.remove_liquidity(250_000).unwrap();

        assert_eq!(receipt.base_amount_out, 250_000);
        assert_eq!(receipt.quote_amount_out, 500_000);
        assert_eq!(receipt.ylp_supply, 750_000);
        assert_eq!(market.base_side.shares.ylp_supply, 750_000);
        assert_eq!(market.quote_side.shares.ylp_supply, 750_000);
    }
