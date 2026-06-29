use super::*;
    use crate::state::{PendingAuthorityChange, PendingConfigChange};
    use crate::{
        constants::{
            INTEREST_INITIAL_RATE_AT_TARGET_NAD, INTEREST_MAX_RATE_AT_TARGET_NAD,
            INTEREST_MIN_RATE_AT_TARGET_NAD, MS_PER_YEAR, NAD, TARGET_MS_PER_SLOT,
        },
        state::{
            Debt, HlpVault, Insurance, MarketConfig, MarketHealth, MarketSide, Reserves, Risk,
        },
    };

    fn slots_for_ms(ms: u64) -> u64 {
        ms / TARGET_MS_PER_SLOT
    }

    fn test_market(base_cash: u64, quote_cash: u64) -> Market {
        let mut base_side = MarketSide::default();
        base_side.reserves = Reserves {
            live_reserve: base_cash,
            cash_reserve: base_cash,
            reserved_liability: 0,
        };
        let mut quote_side = MarketSide::default();
        quote_side.reserves = Reserves {
            live_reserve: quote_cash,
            cash_reserve: quote_cash,
            reserved_liability: 0,
        };
        Market {
            version: 2,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            ylp_mint: Pubkey::new_unique(),
            operator: Pubkey::new_unique(),
            manager: Pubkey::new_unique(),
            base_side,
            quote_side,
            config: MarketConfig::default(),
            debt: Debt {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                base_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                quote_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                last_accrual_slot: 0,
                ..Debt::default()
            },
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
    fn no_time_elapsed_is_a_noop() {
        let mut market = test_market(1_000, 1_000);
        market.debt.last_accrual_slot = 100;
        market.accrue_interest_to_slot(100).unwrap();
        assert_eq!(market.debt.quote_borrow_index_nad, NAD as u128);
        assert_eq!(
            market.debt.quote_rate_at_target_nad,
            INTEREST_INITIAL_RATE_AT_TARGET_NAD
        );
        assert_eq!(market.debt.last_accrual_slot, 100);
    }

    #[test]
    fn idle_side_drifts_anchor_down_toward_min() {
        // Cash present, zero debt -> utilization 0 -> error -1 -> anchor falls.
        let mut market = test_market(1_000_000, 1_000_000);
        market
            .accrue_interest_to_slot(slots_for_ms(MS_PER_YEAR))
            .unwrap();
        assert!(market.debt.quote_rate_at_target_nad < INTEREST_INITIAL_RATE_AT_TARGET_NAD);
        assert!(market.debt.quote_rate_at_target_nad >= INTEREST_MIN_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn high_utilization_raises_anchor_and_accrues_index() {
        // Quote borrowed 950 via base-hLP, 50 cash -> util 95% (above 90% target).
        // error = +0.5 -> curve mult 2.5x -> rate = 4% * 2.5 = 10% APR.
        let mut market = test_market(1_000_000, 50);
        market.base_hlp_vault.debt_shares = 950;
        market
            .accrue_interest_to_slot(slots_for_ms(MS_PER_YEAR))
            .unwrap();
        // 10% APR over one year compounds the index to 1.10.
        assert_eq!(market.debt.quote_borrow_index_nad, (NAD as u128) * 110 / 100);
        // Anchor drifted up (util above target).
        assert!(market.debt.quote_rate_at_target_nad > INTEREST_INITIAL_RATE_AT_TARGET_NAD);
        assert!(market.debt.quote_rate_at_target_nad <= INTEREST_MAX_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn margin_and_hlp_debt_both_count_toward_utilization() {
        // Quote debt = 480 margin + 480 base-hLP = 960 borrowed, 40 cash -> 96%
        // (> target), so the anchor must rise. If either leg were ignored, util
        // would fall below target and the anchor would instead drop.
        let mut market = test_market(1_000_000, 40);
        market.debt.fixed_quote_shares = 480;
        market.base_hlp_vault.debt_shares = 480;
        market
            .accrue_interest_to_slot(slots_for_ms(MS_PER_YEAR))
            .unwrap();
        assert!(market.debt.quote_rate_at_target_nad > INTEREST_INITIAL_RATE_AT_TARGET_NAD);
    }

    #[test]
    fn anchor_saturates_at_max_under_sustained_pressure() {
        // ~100% utilization held for years: the anchor ramps up (capped per
        // step) and clamps at the max, never exceeding it.
        let mut market = test_market(1_000_000, 1);
        market.base_hlp_vault.debt_shares = 10_000;
        for year in 1..=15u64 {
            market
                .accrue_interest_to_slot(slots_for_ms(MS_PER_YEAR * year))
                .unwrap();
        }
        assert_eq!(
            market.debt.quote_rate_at_target_nad,
            INTEREST_MAX_RATE_AT_TARGET_NAD
        );
    }
