use crate::{
    types::{
        omnipair::{self, BorrowInstruction, BorrowInstructionAccounts, BorrowInstructionData},
        AdjustPositionArgs, Pair, UserPosition,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, POSITION_SEED_PREFIX, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn borrow(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }

        let data = self.get_data_borrow();
        let accounts = self.get_accounts_borrow();

        let user_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8);

        if user_position.is_none() {
            // No position found, skip
            return;
        }

        // Capture initial state
        let initial_user_balance = self
            .trident
            .get_token_account(accounts.user_token_account)
            .expect("User token account should exist")
            .account
            .amount;

        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let initial_position = user_position.expect("User position should exist");

        let ix = BorrowInstruction::data(BorrowInstructionData::new(data.clone()))
            .accounts(accounts.clone())
            .instruction();

        let res = self.trident.process_transaction(&[ix], Some("Borrow"));

        // Only verify invariants if transaction succeeded
        // Transaction may fail with expected errors (e.g., BorrowingPowerExceeded)
        if res.is_success() {
            self.verify_borrow_invariants(
                &accounts,
                &initial_pair,
                &initial_position,
                initial_user_balance,
            );
        }
    }

    fn get_data_borrow(&mut self) -> AdjustPositionArgs {
        // Use very small amounts to increase success rate
        // Borrowing power depends on collateral, which may be limited
        // Use weighted distribution: 80% small amounts, 20% larger amounts
        let amount = if self.trident.random_from_range(0..=9) < 8 {
            // 80% chance: very small borrow (10 to 10,000)
            self.trident.random_from_range(10..=10_000)
        } else {
            // 20% chance: larger borrow (10,000 to 100,000)
            self.trident.random_from_range(10_000..=100_000)
        };

        self.trident
            .record_histogram("BORROW_AMOUNT", amount as f64);

        AdjustPositionArgs::new(amount)
    }

    fn get_accounts_borrow(&mut self) -> BorrowInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");

        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair, 8)
            .expect("Pair should exist");

        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        let user_position = self
            .trident
            .find_program_address(
                &[POSITION_SEED_PREFIX, pair.as_ref(), user.as_ref()],
                &omnipair::program_id(),
            )
            .0;

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        let vault_token_mint = if self.trident.random_from_range(0..=1) == 0 {
            pair_account.token0
        } else {
            pair_account.token1
        };

        // let vault_token_mint = pair_account.token1;

        let token_vault =
            self.trident
                .get_associated_token_address(&vault_token_mint, &pair, &TOKEN_PROGRAM);

        let user_token_account =
            self.trident
                .get_associated_token_address(&vault_token_mint, &user, &TOKEN_PROGRAM);

        BorrowInstructionAccounts::new(
            pair,
            user_position,
            pair_account.rate_model,
            futarchy_authority,
            token_vault,
            user_token_account,
            vault_token_mint,
            user,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    fn verify_borrow_invariants(
        &mut self,
        accounts: &BorrowInstructionAccounts,
        initial_pair: &Pair,
        initial_position: &UserPosition,
        initial_user_balance: u64,
    ) {
        // Fetch final state
        let final_user_balance = self
            .trident
            .get_token_account(accounts.user_token_account)
            .expect("User token account should exist")
            .account
            .amount;

        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let final_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8)
            .expect("User position should exist");

        // Determine which token is being borrowed
        let vault_token_account = self
            .trident
            .get_token_account(accounts.token_vault)
            .expect("Token vault should exist")
            .account;
        let is_token0 = vault_token_account.mint == final_pair.token0;

        // Calculate actual borrow amount (may differ from args.amount if u64::MAX was used)
        let actual_borrow_amount = final_user_balance
            .checked_sub(initial_user_balance)
            .expect("User balance should increase");

        // INVARIANT 1: User should have received borrowed tokens
        assert!(
            actual_borrow_amount > 0,
            "User should receive borrowed tokens"
        );

        // INVARIANT 2: Pair's total debt should increase by at least the borrow amount
        // Note: update() accrues interest before borrowing, so total_debt may increase more than just the borrow amount
        let (initial_total_debt, final_total_debt) = if is_token0 {
            (initial_pair.total_debt0, final_pair.total_debt0)
        } else {
            (initial_pair.total_debt1, final_pair.total_debt1)
        };
        let min_expected_debt = initial_total_debt
            .checked_add(actual_borrow_amount)
            .expect("Total debt increase calculation");
        assert!(
            final_total_debt >= min_expected_debt,
            "Pair total debt should be at least initial + borrow amount (was {}, expected >= {})",
            final_total_debt,
            min_expected_debt
        );

        // INVARIANT 3: User position debt shares should increase correctly
        let (initial_debt_shares, final_debt_shares, initial_total_shares, final_total_shares) =
            if is_token0 {
                (
                    initial_position.debt0_shares,
                    final_position.debt0_shares,
                    initial_pair.total_debt0_shares,
                    final_pair.total_debt0_shares,
                )
            } else {
                (
                    initial_position.debt1_shares,
                    final_position.debt1_shares,
                    initial_pair.total_debt1_shares,
                    final_pair.total_debt1_shares,
                )
            };

        // Calculate expected shares increase
        let expected_shares = if initial_total_shares == 0 {
            // First debt: shares = amount (1:1 ratio)
            actual_borrow_amount
        } else {
            // Subsequent debt: shares = amount * total_shares / total_debt
            ((actual_borrow_amount as u128)
                .checked_mul(initial_total_shares as u128)
                .expect("Shares calculation")
                .checked_div(initial_total_debt as u128)
                .expect("Shares division")) as u64
        };

        // Allow for 1-unit rounding tolerance in shares calculation
        let expected_final_debt_shares = initial_debt_shares
            .checked_add(expected_shares)
            .expect("User debt shares increase");
        assert!(
            final_debt_shares.abs_diff(expected_final_debt_shares) <= 1,
            "User debt shares should increase correctly (with 1-unit rounding tolerance). Expected: {}, Got: {}",
            expected_final_debt_shares,
            final_debt_shares
        );

        let expected_final_total_shares = initial_total_shares
            .checked_add(expected_shares)
            .expect("Total debt shares increase");
        assert!(
            final_total_shares.abs_diff(expected_final_total_shares) <= 1,
            "Pair total debt shares should increase correctly (with 1-unit rounding tolerance). Expected: {}, Got: {}",
            expected_final_total_shares,
            final_total_shares
        );

        if is_token0 {
            assert!(
                final_position.debt0_shares > initial_position.debt0_shares,
                "Debt0 shares should increase {:?} > {:?}",
                final_position.debt0_shares,
                initial_position.debt0_shares
            );
        } else {
            assert!(
                final_position.debt1_shares > initial_position.debt1_shares,
                "Debt1 shares should increase {:?} > {:?}",
                final_position.debt1_shares,
                initial_position.debt1_shares
            );
        }

        // INVARIANT 4: Vault solvency check - vault balance >= reserves + collateral - debt
        let vault0_balance = self.trident.get_associated_token_address(
            &final_pair.token0,
            &accounts.pair,
            &TOKEN_PROGRAM,
        );
        let vault0_amount = self
            .trident
            .get_token_account(vault0_balance)
            .expect("Token0 vault should exist")
            .account
            .amount;

        let vault1_balance = self.trident.get_associated_token_address(
            &final_pair.token1,
            &accounts.pair,
            &TOKEN_PROGRAM,
        );
        let vault1_amount = self
            .trident
            .get_token_account(vault1_balance)
            .expect("Token1 vault should exist")
            .account
            .amount;

        let required0 = final_pair
            .reserve0
            .checked_add(final_pair.total_collateral0)
            .expect("Reserve + collateral overflow")
            .saturating_sub(final_pair.total_debt0);
        assert!(
            vault0_amount >= required0,
            "Token0 vault balance must be >= reserve0 + collateral0 - debt0"
        );

        let required1 = final_pair
            .reserve1
            .checked_add(final_pair.total_collateral1)
            .expect("Reserve + collateral overflow")
            .saturating_sub(final_pair.total_debt1);
        assert!(
            vault1_amount >= required1,
            "Token1 vault balance must be >= reserve1 + collateral1 - debt1"
        );

        // INVARIANT 5: Verify user position ownership hasn't changed
        assert_eq!(
            final_position.owner, accounts.user,
            "User position owner should match user"
        );
        assert_eq!(
            final_position.pair, accounts.pair,
            "User position pair should match pair"
        );

        // INVARIANT 6: Collateral amounts should not change during borrow
        assert_eq!(
            final_position.collateral0, initial_position.collateral0,
            "Collateral0 should not change during borrow"
        );
        assert_eq!(
            final_position.collateral1, initial_position.collateral1,
            "Collateral1 should not change during borrow"
        );

        // INVARIANT 7: on success, token_vault must be the canonical ATA for (pair, vault_token_mint)
        let canonical_vault = self.trident.get_associated_token_address(
            &accounts.vault_token_mint,
            &accounts.pair,
            &TOKEN_PROGRAM,
        );
        assert_eq!(
            accounts.token_vault, canonical_vault,
            "Borrow accepted a non-canonical token_vault for the pair and mint"
        );
    }
}
