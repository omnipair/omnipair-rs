

use crate::{
    types::{
        omnipair::{
            self, RemoveCollateralInstruction, RemoveCollateralInstructionAccounts,
            RemoveCollateralInstructionData,
        },
        AdjustPositionArgs, Pair, UserPosition,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, POSITION_SEED_PREFIX, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn remove_collateral(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }

        let data = self.get_data_remove_collateral();
        let accounts = self.get_accounts_remove_collateral();

        // Check if user position exists
        let initial_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8);

        if initial_position.is_none() {
            // No position found, skip
            return;
        }

        // Capture initial state
        let initial_user_balance = self
            .trident
            .get_token_account(accounts.user_token_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let ix =
            RemoveCollateralInstruction::data(RemoveCollateralInstructionData::new(data.clone()))
                .accounts(accounts.clone())
                .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Remove Collateral"));

        // Only verify invariants if transaction succeeded
        // Transaction may fail with expected errors (e.g., InsufficientCollateral, BorrowingPowerExceeded)
        if res.is_success() {
            self.verify_remove_collateral_invariants(
                &data,
                &accounts,
                &initial_pair,
                &initial_position.unwrap(),
                initial_user_balance,
            );
        }
    }

    fn get_data_remove_collateral(&mut self) -> AdjustPositionArgs {
        // Use smaller amounts more likely to match actual collateral amounts
        let amount = self.trident.random_from_range(100..=1_000_000);
        self.trident
            .record_histogram("REMOVE_COLLATERAL_AMOUNT", amount as f64);
        AdjustPositionArgs::new(amount)
    }

    fn get_accounts_remove_collateral(&mut self) -> RemoveCollateralInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");

        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair, 8)
            .unwrap();

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

        let token_vault =
            self.trident
                .get_associated_token_address(&vault_token_mint, &pair, &TOKEN_PROGRAM);

        let user_token_account =
            self.trident
                .get_associated_token_address(&vault_token_mint, &user, &TOKEN_PROGRAM);

        RemoveCollateralInstructionAccounts::new(
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

    fn verify_remove_collateral_invariants(
        &mut self,
        args: &AdjustPositionArgs,
        accounts: &RemoveCollateralInstructionAccounts,
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

        // Determine which token is being withdrawn
        let vault_token_account = self
            .trident
            .get_token_account(accounts.token_vault)
            .expect("Token vault should exist")
            .account;
        let is_token0 = vault_token_account.mint == final_pair.token0;

        // Get initial collateral amount
        let initial_collateral = if is_token0 {
            initial_position.collateral0
        } else {
            initial_position.collateral1
        };

        // Calculate actual withdraw amount
        // If args.amount == u64::MAX, the program withdraws all available collateral
        let is_withdraw_all = args.amount == u64::MAX;
        let actual_withdraw_amount = if is_withdraw_all {
            initial_collateral
        } else {
            args.amount
        };

        // INVARIANT 1: User token balance should increase by exactly the withdraw amount
        let amount_transferred = final_user_balance
            .checked_sub(initial_user_balance)
            .expect("User balance should increase");
        assert_eq!(
            amount_transferred, actual_withdraw_amount,
            "User should receive exactly the withdrawn amount"
        );

        // INVARIANT 2: User position collateral should decrease by exactly the withdraw amount
        let final_collateral = if is_token0 {
            final_position.collateral0
        } else {
            final_position.collateral1
        };
        assert_eq!(
            final_collateral,
            initial_collateral
                .checked_sub(actual_withdraw_amount)
                .expect("Collateral decrease calculation"),
            "User position collateral should decrease by withdraw amount"
        );

        // INVARIANT 3: Pair total collateral should decrease by exactly the withdraw amount
        let (initial_total_collateral, final_total_collateral) = if is_token0 {
            (initial_pair.total_collateral0, final_pair.total_collateral0)
        } else {
            (initial_pair.total_collateral1, final_pair.total_collateral1)
        };
        assert_eq!(
            final_total_collateral,
            initial_total_collateral
                .checked_sub(actual_withdraw_amount)
                .expect("Total collateral decrease calculation"),
            "Pair total collateral should decrease by withdraw amount"
        );

        // INVARIANT 4: If withdraw_all was requested, verify final collateral is 0
        // (this assumes no debt that would prevent full withdrawal - which is validated in the program)
        if is_withdraw_all && actual_withdraw_amount == initial_collateral {
            assert_eq!(
                final_collateral, 0,
                "When withdrawing all available collateral, final collateral should be 0"
            );
        }

        // INVARIANT 5: Vault solvency - vaults must hold at least reserves + total_collateral - debt
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

        // INVARIANT 6: Verify user position ownership hasn't changed
        assert_eq!(
            final_position.owner, accounts.user,
            "User position owner should match user"
        );
        assert_eq!(
            final_position.pair, accounts.pair,
            "User position pair should match pair"
        );

        // INVARIANT 7: Debt amounts should not change during collateral removal
        assert_eq!(
            final_position.debt0_shares, initial_position.debt0_shares,
            "Debt0 shares should not change when removing collateral"
        );
        assert_eq!(
            final_position.debt1_shares, initial_position.debt1_shares,
            "Debt1 shares should not change when removing collateral"
        );

        // INVARIANT 8: Non-withdrawn collateral unchanged
        if is_token0 {
            assert_eq!(
                final_position.collateral1, initial_position.collateral1,
                "Non-withdrawn collateral (token1) should not change"
            );
        } else {
            assert_eq!(
                final_position.collateral0, initial_position.collateral0,
                "Non-withdrawn collateral (token0) should not change"
            );
        }

        // INVARIANT 9: on success, token_vault must be the canonical ATA for (pair, vault_token_mint)
        let canonical_vault = self.trident.get_associated_token_address(
            &accounts.vault_token_mint,
            &accounts.pair,
            &TOKEN_PROGRAM,
        );
        assert_eq!(
            accounts.token_vault, canonical_vault,
            "RemoveCollateral accepted a non-canonical token_vault for the pair and mint"
        );
    }
}
