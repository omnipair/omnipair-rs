use trident_fuzz::fuzzing::LAMPORTS_PER_SOL;

use crate::{
    types::{
        omnipair::{
            self, LiquidateInstruction, LiquidateInstructionAccounts, LiquidateInstructionData,
        },
        Pair, UserPosition,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn liquidate(&mut self) {
        if self.fuzz_accounts.pair.is_empty() || self.fuzz_accounts.user_position.is_empty() {
            return;
        }

        let accounts = self.get_accounts_liquidate();

        // Capture initial state before the transaction
        let initial_caller_balance = self
            .trident
            .get_token_account(accounts.caller_token_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let initial_position = self
            .trident
            .get_account_with_type::<UserPosition>(&accounts.user_position, 8)
            .expect("User position should exist");

        let ix = LiquidateInstruction::data(LiquidateInstructionData::new())
            .accounts(accounts.clone())
            .instruction();

        let res = self.trident.process_transaction(&[ix], Some("Liquidate"));

        // Only verify invariants if transaction succeeded
        // Transaction may fail with expected errors (e.g., NotUndercollateralized, no debt)
        if res.is_success() {
            self.verify_liquidate_invariants(
                &accounts,
                &initial_pair,
                &initial_position,
                initial_caller_balance,
            );
        }
    }

    fn get_accounts_liquidate(&mut self) -> LiquidateInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");

        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair, 8)
            .expect("Pair should exist");

        let user_position = self.fuzz_accounts.user_position.get(&mut self.trident).expect("User position should exist");
        let user_position_account_data = self
            .trident
            .get_account_with_type::<UserPosition>(&user_position, 8)
            .expect("User position should exist");

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        let collateral_token_mint = if self.trident.random_from_range(0..=1) == 0 {
            pair_account.token0
        } else {
            pair_account.token1
        };

        let collateral_vault = self.trident.get_associated_token_address(
            &collateral_token_mint,
            &pair,
            &TOKEN_PROGRAM,
        );

        let caller = self.fuzz_accounts.caller.insert(&mut self.trident, None);
        self.trident.airdrop(
            &caller,
            LAMPORTS_PER_SOL.checked_mul(2).expect("Airdrop amount"),
        );
        let caller_token_account = self.trident.get_associated_token_address(
            &collateral_token_mint,
            &caller,
            &TOKEN_PROGRAM,
        );
        self.trident
            .initialize_associated_token_account(&caller, &collateral_token_mint, &caller);

        LiquidateInstructionAccounts::new(
            pair,
            user_position,
            pair_account.rate_model,
            futarchy_authority,
            collateral_vault,
            caller_token_account,
            collateral_token_mint,
            user_position_account_data.owner,
            caller,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    fn verify_liquidate_invariants(
        &mut self,
        accounts: &LiquidateInstructionAccounts,
        initial_pair: &Pair,
        initial_position: &UserPosition,
        initial_caller_balance: u64,
    ) {
        // Fetch final state
        let final_caller_balance = self
            .trident
            .get_token_account(accounts.caller_token_account)
            .expect("Caller token account should exist")
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

        // Determine which token is collateral and which is debt
        let collateral_vault = self
            .trident
            .get_token_account(accounts.collateral_vault)
            .expect("Collateral vault should exist")
            .account;
        let is_collateral_token0 = collateral_vault.mint == final_pair.token0;

        // Calculate initial debt
        let (_, initial_debt_shares) = if is_collateral_token0 {
            // Collateral is token0, debt is token1
            let debt = if initial_pair.total_debt1_shares == 0 {
                0
            } else {
                ((initial_position.debt1_shares as u128)
                    .checked_mul(initial_pair.total_debt1 as u128)
                    .expect("Debt calculation")
                    .checked_div(initial_pair.total_debt1_shares as u128)
                    .expect("Debt division")) as u64
            };
            (debt, initial_position.debt1_shares)
        } else {
            // Collateral is token1, debt is token0
            let debt = if initial_pair.total_debt0_shares == 0 {
                0
            } else {
                ((initial_position.debt0_shares as u128)
                    .checked_mul(initial_pair.total_debt0 as u128)
                    .expect("Debt calculation")
                    .checked_div(initial_pair.total_debt0_shares as u128)
                    .expect("Debt division")) as u64
            };
            (debt, initial_position.debt0_shares)
        };

        // INVARIANT 1: Caller received liquidation incentive (should be > 0)
        let caller_incentive = final_caller_balance
            .checked_sub(initial_caller_balance)
            .expect("Caller balance should increase");
        assert!(
            caller_incentive > 0,
            "Liquidator should receive incentive for liquidating"
        );

        // INVARIANT 2: User position collateral decreased
        let (initial_collateral, final_collateral) = if is_collateral_token0 {
            (initial_position.collateral0, final_position.collateral0)
        } else {
            (initial_position.collateral1, final_position.collateral1)
        };
        let collateral_seized = initial_collateral
            .checked_sub(final_collateral)
            .expect("Collateral should decrease");
        assert!(
            collateral_seized > 0,
            "Position collateral should be seized during liquidation"
        );

        // INVARIANT 3: Pair total collateral decreased by the seized amount
        let (initial_total_collateral, final_total_collateral) = if is_collateral_token0 {
            (initial_pair.total_collateral0, final_pair.total_collateral0)
        } else {
            (initial_pair.total_collateral1, final_pair.total_collateral1)
        };
        assert_eq!(
            final_total_collateral,
            initial_total_collateral
                .checked_sub(collateral_seized)
                .expect("Total collateral decrease"),
            "Pair total collateral should decrease by seized amount"
        );

        // INVARIANT 4: Collateral seized >= caller incentive (caller gets incentive, rest to reserves)
        assert!(
            collateral_seized >= caller_incentive,
            "Collateral seized must cover at least the liquidation incentive"
        );

        // INVARIANT 5: Debt decreased or was written off (final debt <= initial debt)
        let (_, final_debt_shares) = if is_collateral_token0 {
            let debt = if final_pair.total_debt1_shares == 0 {
                0
            } else {
                ((final_position.debt1_shares as u128)
                    .checked_mul(final_pair.total_debt1 as u128)
                    .expect("Debt calculation")
                    .checked_div(final_pair.total_debt1_shares as u128)
                    .expect("Debt division")) as u64
            };
            (debt, final_position.debt1_shares)
        } else {
            let debt = if final_pair.total_debt0_shares == 0 {
                0
            } else {
                ((final_position.debt0_shares as u128)
                    .checked_mul(final_pair.total_debt0 as u128)
                    .expect("Debt calculation")
                    .checked_div(final_pair.total_debt0_shares as u128)
                    .expect("Debt division")) as u64
            };
            (debt, final_position.debt0_shares)
        };

        // Note: final_debt may not be less than initial_debt if significant interest accrued during update()
        // The authoritative measure is debt_shares, which must decrease during liquidation
        assert!(
            final_debt_shares < initial_debt_shares,
            "Debt shares must decrease during liquidation (was {}, now {})",
            initial_debt_shares, final_debt_shares
        );

        // INVARIANT 6: Collateral reserve increased (by collateral_seized - caller_incentive)
        let (initial_collateral_reserve, final_collateral_reserve) = if is_collateral_token0 {
            (initial_pair.reserve0, final_pair.reserve0)
        } else {
            (initial_pair.reserve1, final_pair.reserve1)
        };

        // Note: Due to pair.update(), reserves may have interest accrued, so we can't do exact check
        // But collateral reserve should have increased
        assert!(
            final_collateral_reserve >= initial_collateral_reserve,
            "Collateral reserve should increase (seized collateral - incentive goes to reserves)"
        );

        // INVARIANT 7: Vault solvency check
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

        // INVARIANT 8: Position ownership unchanged
        assert_eq!(
            final_position.owner, initial_position.owner,
            "Position owner should not change"
        );
        assert_eq!(
            final_position.pair, initial_position.pair,
            "Position pair should not change"
        );

        // INVARIANT 9: Non-seized collateral unchanged
        if is_collateral_token0 {
            assert_eq!(
                final_position.collateral1, initial_position.collateral1,
                "Non-seized collateral (token1) should not change"
            );
        } else {
            assert_eq!(
                final_position.collateral0, initial_position.collateral0,
                "Non-seized collateral (token0) should not change"
            );
        }
    }
}
