use crate::{
    types::{
        omnipair::{
            self, AddCollateralInstruction, AddCollateralInstructionAccounts,
            AddCollateralInstructionData,
        },
        AdjustPositionArgs, Pair, UserPosition,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, POSITION_SEED_PREFIX, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn add_collateral(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }

        let data = self.get_data_add_collateral();
        let accounts = self.get_accounts_add_collateral();

        // Store initial state
        let initial_user_balance = self
            .trident
            .get_token_account(accounts.user_collateral_token_account)
            .expect("User collateral account should exist")
            .account
            .amount;

        // Get initial pair and position state (position may not exist yet - init_if_needed)
        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let initial_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8);

        let ix = AddCollateralInstruction::data(AddCollateralInstructionData::new(data.clone()))
            .accounts(accounts.clone())
            .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Add Collateral"));

        if res.is_success() {
            self.verify_add_collateral_invariants(
                &data,
                &accounts,
                initial_user_balance,
                &initial_pair,
                initial_position.as_ref(),
            );
        }
    }

    fn get_accounts_add_collateral(&mut self) -> AddCollateralInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");

        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair, 8)
            .expect("Pair should exist");

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // user related accounts
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");
        let user_position = self
            .trident
            .find_program_address(
                &[POSITION_SEED_PREFIX, pair.as_ref(), user.as_ref()],
                &omnipair::program_id(),
            )
            .0;

        let collateral_token_mint = if self.trident.random_from_range(0..=1) == 0 {
            pair_account.token0
        } else {
            pair_account.token1
        };

        // let collateral_token_mint = pair_account.token0;

        let collateral_vault = self.trident.get_associated_token_address(
            &collateral_token_mint,
            &pair,
            &TOKEN_PROGRAM,
        );

        let user_collateral_token_account = self.trident.get_associated_token_address(
            &collateral_token_mint,
            &user,
            &TOKEN_PROGRAM,
        );

        AddCollateralInstructionAccounts::new(
            pair,
            pair_account.rate_model,
            futarchy_authority,
            user_position,
            collateral_vault,
            user_collateral_token_account,
            collateral_token_mint,
            user,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    fn get_data_add_collateral(&mut self) -> AdjustPositionArgs {
        // Use reasonable collateral amounts that match typical use cases
        let amount = self.trident.random_from_range(100_000..=100_000_000);

        self.trident
            .record_histogram("ADD_COLLATERAL_AMOUNT", amount as f64);
        AdjustPositionArgs::new(amount)
    }

    fn verify_add_collateral_invariants(
        &mut self,
        args: &AdjustPositionArgs,
        accounts: &AddCollateralInstructionAccounts,
        initial_user_balance: u64,
        initial_pair: &Pair,

        initial_position: Option<&UserPosition>,
    ) {
        // Get final pair state
        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist after add collateral");

        // Get user position
        let user_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8)
            .expect("User position should exist after add collateral");

        // Determine which token is being used as collateral
        let is_token0 = accounts.collateral_token_mint == final_pair.token0;

        // Verify user balance decreased by exactly the amount
        let final_user_balance = self
            .trident
            .get_token_account(accounts.user_collateral_token_account)
            .expect("User collateral account should exist")
            .account
            .amount;

        let amount_transferred = initial_user_balance
            .checked_sub(final_user_balance)
            .expect("User balance should decrease");
        assert_eq!(
            amount_transferred, args.amount,
            "User balance should decrease by exactly the collateral amount"
        );

        // Verify user position collateral increased by EXACTLY the amount
        // (position may be newly created, so initial collateral is 0 if it didn't exist)
        if is_token0 {
            let initial_collateral = initial_position.map(|p| p.collateral0).unwrap_or(0);
            let expected_collateral = initial_collateral
                .checked_add(args.amount)
                .expect("User position collateral0 should not overflow");
            assert_eq!(
                user_position.collateral0, expected_collateral,
                "User position collateral0 should increase by exactly the amount added"
            );
        } else {
            let initial_collateral = initial_position.map(|p| p.collateral1).unwrap_or(0);
            let expected_collateral = initial_collateral
                .checked_add(args.amount)
                .expect("User position collateral1 should not overflow");
            assert_eq!(
                user_position.collateral1, expected_collateral,
                "User position collateral1 should increase by exactly the amount added"
            );
        }

        // Verify pair total_collateral increased by EXACTLY the amount (atomic transaction)
        if is_token0 {
            let expected_total = initial_pair
                .total_collateral0
                .checked_add(args.amount)
                .expect("Total collateral0 should not overflow");
            assert_eq!(
                final_pair.total_collateral0, expected_total,
                "Pair total_collateral0 should increase by exactly the amount added"
            );
        } else {
            let expected_total = initial_pair
                .total_collateral1
                .checked_add(args.amount)
                .expect("Total collateral1 should not overflow");
            assert_eq!(
                final_pair.total_collateral1, expected_total,
                "Pair total_collateral1 should increase by exactly the amount added"
            );
        }

        // Critical accounting invariant: vault balance should be >= reserves + collateral - debt
        // (vaults hold LP liquidity + borrower collateral, minus what was borrowed out)
        let vault_balance = self
            .trident
            .get_token_account(accounts.collateral_vault)
            .expect("Collateral vault should exist")
            .account
            .amount;

        if is_token0 {
            let total_required = final_pair
                .reserve0
                .checked_add(final_pair.total_collateral0)
                .expect("Reserve + collateral overflow")
                .saturating_sub(final_pair.total_debt0); // Subtract debt (borrowed tokens are out of vault)
            assert!(
                vault_balance >= total_required,
                "Vault balance must be >= reserve0 + total_collateral0 - total_debt0"
            );
        } else {
            let total_required = final_pair
                .reserve1
                .checked_add(final_pair.total_collateral1)
                .expect("Reserve + collateral overflow")
                .saturating_sub(final_pair.total_debt1); // Subtract debt (borrowed tokens are out of vault)
            assert!(
                vault_balance >= total_required,
                "Vault balance must be >= reserve1 + total_collateral1 - total_debt1"
            );
        }

        // Verify user position is properly initialized (always, whether new or existing)
        assert_eq!(
            user_position.owner, accounts.user,
            "User position owner should match user"
        );
        assert_eq!(
            user_position.pair, accounts.pair,
            "User position pair should match pair"
        );
    }
}
