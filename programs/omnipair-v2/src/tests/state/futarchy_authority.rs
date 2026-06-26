use super::*;

    #[test]
    fn default_auction_split_routes_all_protocol_revenue_to_fee_lane() {
        let split = ProtocolAuctionSplit::default();

        assert_eq!(split.fee_auction_bps, BPS_DENOMINATOR);
        assert_eq!(split.buyback_auction_bps, 0);
        assert!(split.is_valid());
    }

    #[test]
    fn auction_params_reject_invalid_curve_shapes() {
        let mut params = ProtocolAuctionParams::default_epoch();
        params.validate().unwrap();

        params.floor_multiplier_bps = params.start_multiplier_bps + 1;
        assert_eq!(
            params.validate().unwrap_err(),
            error!(ErrorCode::InvalidAuctionConfig)
        );

        params = ProtocolAuctionParams::default_epoch();
        params.duration_slots = 0;
        assert_eq!(
            params.validate().unwrap_err(),
            error!(ErrorCode::InvalidAuctionConfig)
        );
    }

    #[test]
    fn initialized_authority_uses_treasury_only_auction_recipients() {
        let authority = Pubkey::new_unique();
        let treasury = Pubkey::new_unique();
        let buybacks_vault = Pubkey::new_unique();
        let team_treasury = Pubkey::new_unique();
        let staking_vault = Pubkey::new_unique();
        let fee_accepted_mint = Pubkey::new_unique();
        let buyback_accepted_mint = Pubkey::new_unique();

        let futarchy = FutarchyAuthority::initialize(
            authority,
            100,
            200,
            treasury,
            buybacks_vault,
            team_treasury,
            staking_vault,
            fee_accepted_mint,
            buyback_accepted_mint,
            BPS_DENOMINATOR,
            0,
            0,
            123,
            42,
        )
        .unwrap();

        assert_eq!(futarchy.fee_auction.accepted_mint, fee_accepted_mint);
        assert_eq!(
            futarchy.buyback_auction.accepted_mint,
            buyback_accepted_mint
        );
        assert_eq!(futarchy.fee_auction.recipients.treasury, treasury);
        assert_eq!(futarchy.fee_auction.recipients.staking_vault, staking_vault);
        assert_eq!(
            futarchy.fee_auction.recipients.treasury_bps,
            BPS_DENOMINATOR
        );
        assert_eq!(futarchy.fee_auction.recipients.staking_vault_bps, 0);
        assert_eq!(futarchy.fee_auction.last_settlement_slot, 123);
        futarchy.validate().unwrap();
    }
