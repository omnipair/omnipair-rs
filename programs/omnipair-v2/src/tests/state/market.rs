use super::*;

    fn market_with_roles(manager: Pubkey, operator: Pubkey) -> Market {
        Market {
            version: MARKET_VERSION,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            ylp_mint: Pubkey::new_unique(),
            operator,
            manager,
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
    fn assert_manager_accepts_only_the_manager() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let market = market_with_roles(manager, operator);
        assert!(market.assert_manager(manager).is_ok());
        // The operator is NOT the manager for sensitive (manager-only) actions.
        assert!(market.assert_manager(operator).is_err());
        assert!(market.assert_manager(Pubkey::new_unique()).is_err());
    }

    #[test]
    fn assert_config_authority_accepts_only_manager() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let market = market_with_roles(manager, operator);
        assert!(market.assert_config_authority(manager).is_ok());
        assert!(market.assert_config_authority(operator).is_err());
        assert!(market
            .assert_config_authority(Pubkey::new_unique())
            .is_err());
    }

    #[test]
    fn operator_rotation_requires_timelock() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let new_operator = Pubkey::new_unique();
        let mut market = market_with_roles(manager, operator);

        let action = market
            .prepare_operator_update(manager, new_operator, 10)
            .unwrap();
        assert_eq!(
            action,
            MarketTimelockAction::Scheduled {
                execute_after_slot: 10 + MARKET_GOVERNANCE_DELAY_SLOTS
            }
        );
        assert_eq!(market.operator, operator);

        let err = market
            .prepare_operator_update(
                manager,
                new_operator,
                10 + MARKET_GOVERNANCE_DELAY_SLOTS - 1,
            )
            .unwrap_err();
        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::GovernanceTimelockNotReady)
        );

        let action = market
            .prepare_operator_update(manager, new_operator, 10 + MARKET_GOVERNANCE_DELAY_SLOTS)
            .unwrap();
        assert_eq!(action, MarketTimelockAction::Ready);
        market.apply_operator_update(new_operator);
        assert_eq!(market.operator, new_operator);
        assert!(!market.pending_operator.active);
    }

    #[test]
    fn config_update_requires_timelock() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let mut market = market_with_roles(manager, operator);
        let mut config = MarketConfig::default();
        config.target_hlp_leverage_bps = BPS_DENOMINATOR * 2;
        config.recognized_collateral_cap_bps = BPS_DENOMINATOR;
        config.market_health_min_bps = BPS_DENOMINATOR;
        config.ema_half_life_ms = MIN_HALF_LIFE_MS;
        config.directional_ema_half_life_ms = MIN_HALF_LIFE_MS;
        config.k_ema_half_life_ms = MIN_HALF_LIFE_MS;
        config.liquidation_auction_duration_slots = 1_200;
        config.liquidation_auction_start_incentive_bps = 0;

        let action = market.prepare_config_update(manager, config, 7).unwrap();
        assert_eq!(
            action,
            MarketTimelockAction::Scheduled {
                execute_after_slot: 7 + MARKET_GOVERNANCE_DELAY_SLOTS
            }
        );

        let action = market
            .prepare_config_update(manager, config, 7 + MARKET_GOVERNANCE_DELAY_SLOTS)
            .unwrap();
        assert_eq!(action, MarketTimelockAction::Ready);
    }
