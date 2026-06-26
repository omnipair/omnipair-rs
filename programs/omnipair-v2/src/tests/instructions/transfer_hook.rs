use super::*;
    use crate::constants::NAD;

    #[test]
    fn reconstructs_pre_transfer_balances_from_post_transfer_state() {
        let balances = pre_transfer_balances(700, 350, 50).unwrap();
        assert_eq!(
            balances,
            TransferBalances {
                source_pre_balance: 750,
                destination_pre_balance: 300
            }
        );
    }

    #[test]
    fn rejects_post_transfer_destination_underflow() {
        let result = pre_transfer_balances(700, 49, 50);
        assert!(result.is_err());
    }

    #[test]
    fn checkpoints_yield_account_with_pre_transfer_balance() {
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let mut yield_account = YieldAccount {
            owner: Pubkey::default(),
            market: Pubkey::default(),
            asset_mint: Pubkey::default(),
            token_kind: 0,
            recipient: Pubkey::default(),
            swap_fee_checkpoint_nad: 0,
            interest_checkpoint_nad: 0,
            accrued_swap_fee_amount: 0,
            accrued_interest_amount: 0,
            bump: 0,
        };
        yield_account.initialize(owner, market, asset_mint, YieldTokenKind::Ylp, owner, 255);
        let yield_context = YieldContext {
            asset_mint,
            token_kind: YieldTokenKind::Ylp,
            swap_fee_growth_index_nad: 3 * NAD as u128,
            interest_growth_index_nad: 2 * NAD as u128,
        };

        checkpoint_yield_account_state(&mut yield_account, yield_context, 10).unwrap();

        assert_eq!(yield_account.accrued_swap_fee_amount, 30);
        assert_eq!(yield_account.accrued_interest_amount, 20);
        assert_eq!(
            yield_account.swap_fee_checkpoint_nad,
            yield_context.swap_fee_growth_index_nad
        );
        assert_eq!(
            yield_account.interest_checkpoint_nad,
            yield_context.interest_growth_index_nad
        );
    }

    #[test]
    fn accepts_only_canonical_yield_account_pda() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (yield_account_key, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );

        validate_yield_account_pda(
            &yield_account_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump,
        )
        .unwrap();
    }

    #[test]
    fn rejects_matching_yield_account_data_at_wrong_address() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (_, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );
        let wrong_key = Pubkey::new_unique();

        let err = validate_yield_account_pda(
            &wrong_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump,
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidYieldAccount));
    }

    #[test]
    fn rejects_matching_yield_account_data_with_wrong_bump() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (yield_account_key, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );

        let err = validate_yield_account_pda(
            &yield_account_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump.wrapping_add(1),
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidYieldAccount));
    }
