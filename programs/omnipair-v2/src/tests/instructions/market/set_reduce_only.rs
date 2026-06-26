use super::*;

    #[test]
    fn set_market_reduce_only_uses_v1_emergency_authority() {
        assert_ne!(REDUCE_ONLY_EMERGENCY_AUTHORITY, Pubkey::default());
    }
