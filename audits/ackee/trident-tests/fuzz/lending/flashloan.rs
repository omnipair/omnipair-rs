use trident_fuzz::fuzzing::AccountMeta;

use crate::{
    types::{
        omnipair::{
            self, FlashloanInstruction, FlashloanInstructionAccounts, FlashloanInstructionData,
        },
        FlashloanArgs, Pair,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, FLASHLOAN_CALLBACK_RECEIVER_PROGRAM, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn flashloan(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }

        // NOTE: Flashloan failures (InsufficientAmount0/1) are expected and normal during fuzzing
        // The callback receiver program needs to repay the loan + fees, which may not always succeed
        let data = self.get_data_flashloan();
        let accounts = self.get_accounts_flashloan();

        // Capture initial state before the transaction
        let initial_vault0_balance = self
            .trident
            .get_token_account(accounts.token0_vault)
            .expect("Token0 vault should exist")
            .account
            .amount;

        let initial_vault1_balance = self
            .trident
            .get_token_account(accounts.token1_vault)
            .expect("Token1 vault should exist")
            .account
            .amount;

        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let ix = FlashloanInstruction::data(data.clone())
            .accounts(accounts.clone())
            .remaining_accounts(vec![
                AccountMeta::new(accounts.token0_vault, false),
                AccountMeta::new(accounts.token1_vault, false),
            ])
            .instruction();

        let res = self.trident.process_transaction(&[ix], Some("Flashloan"));

        // Only verify invariants if transaction succeeded
        // Transaction may fail with expected errors (e.g., BorrowExceedsReserve, InsufficientAmount)
        if res.is_success() {
            self.verify_flashloan_invariants(
                &data.args,
                &accounts,
                &initial_pair,
                initial_vault0_balance,
                initial_vault1_balance,
            );
        }
    }

    pub fn get_accounts_flashloan(&mut self) -> FlashloanInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair, 8)
            .expect("Pair should exist");

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        let token0_vault =
            self.trident
                .get_associated_token_address(&pair_account.token0, &pair, &TOKEN_PROGRAM);

        let token1_vault =
            self.trident
                .get_associated_token_address(&pair_account.token1, &pair, &TOKEN_PROGRAM);

        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        let receiver_token0_account =
            self.trident
                .get_associated_token_address(&pair_account.token0, &user, &TOKEN_PROGRAM);

        let receiver_token1_account =
            self.trident
                .get_associated_token_address(&pair_account.token1, &user, &TOKEN_PROGRAM);

        FlashloanInstructionAccounts::new(
            pair,
            pair_account.rate_model,
            futarchy_authority,
            token0_vault,
            token1_vault,
            pair_account.token0,
            pair_account.token1,
            receiver_token0_account,
            receiver_token1_account,
            FLASHLOAN_CALLBACK_RECEIVER_PROGRAM,
            user,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    pub fn get_data_flashloan(&mut self) -> FlashloanInstructionData {
        // Strategy: Use very small amounts to maximize success rate
        // Flashloans require the callback receiver to repay loan + fees, which often fails in fuzzing
        let (amount0, amount1) = match self.trident.random_from_range(0..=100) {
            // 40% - Flash only token0 (very small amount)
            0..=39 => (self.trident.random_from_range(100..=1_000), 0),
            // 40% - Flash only token1 (very small amount)
            40..=79 => (0, self.trident.random_from_range(100..=1_000)),
            // 15% - Flash both tokens (very small amounts)
            80..=94 => (
                self.trident.random_from_range(100..=500),
                self.trident.random_from_range(100..=500),
            ),
            // 5% - Slightly larger amounts (stress test)
            _ => (
                self.trident.random_from_range(1_000..=5_000),
                self.trident.random_from_range(1_000..=5_000),
            ),
        };

        self.trident
            .record_histogram("FLASHLOAN_AMOUNT0", amount0 as f64);
        self.trident
            .record_histogram("FLASHLOAN_AMOUNT1", amount1 as f64);

        FlashloanInstructionData::new(FlashloanArgs::new(amount0, amount1, vec![]))
    }

    fn verify_flashloan_invariants(
        &mut self,
        args: &FlashloanArgs,
        accounts: &FlashloanInstructionAccounts,
        initial_pair: &Pair,
        initial_vault0_balance: u64,
        initial_vault1_balance: u64,
    ) {
        // Fetch final state
        let final_vault0_balance = self
            .trident
            .get_token_account(accounts.token0_vault)
            .expect("Token0 vault should exist")
            .account
            .amount;

        let final_vault1_balance = self
            .trident
            .get_token_account(accounts.token1_vault)
            .expect("Token1 vault should exist")
            .account
            .amount;

        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        // Calculate expected fees (FLASHLOAN_FEE_BPS = 5 bps = 0.05%)
        const FLASHLOAN_FEE_BPS: u64 = 5;
        const BPS_DENOMINATOR: u64 = 10_000;

        let fee0 = (args.amount0 as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .expect("Fee0 calculation")
            .checked_div(BPS_DENOMINATOR as u128)
            .expect("Fee0 division") as u64;

        let fee1 = (args.amount1 as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .expect("Fee1 calculation")
            .checked_div(BPS_DENOMINATOR as u128)
            .expect("Fee1 division") as u64;

        // INVARIANT 1: Vault balances must have increased by at least the fee
        // Critical: This ensures flashloan was repaid with fee
        let required_balance0 = initial_vault0_balance
            .checked_add(fee0)
            .expect("Required balance0 calculation");
        let required_balance1 = initial_vault1_balance
            .checked_add(fee1)
            .expect("Required balance1 calculation");

        assert!(
            final_vault0_balance >= required_balance0,
            "Token0 vault must have at least initial balance + fee after flashloan (repayment check)"
        );
        assert!(
            final_vault1_balance >= required_balance1,
            "Token1 vault must have at least initial balance + fee after flashloan (repayment check)"
        );

        // INVARIANT 2: Net profit check - vault balances increased (fees were collected)
        // Only check if fee > 0 (small amounts can round to 0 fee)
        if fee0 > 0 {
            assert!(
                final_vault0_balance > initial_vault0_balance,
                "Token0 vault should have more tokens after flashloan (fee collected)"
            );
        }
        if fee1 > 0 {
            assert!(
                final_vault1_balance > initial_vault1_balance,
                "Token1 vault should have more tokens after flashloan (fee collected)"
            );
        }

        // INVARIANT 3: Vault solvency check - vaults must hold at least reserves + collateral - debt
        let required0 = final_pair
            .reserve0
            .checked_add(final_pair.total_collateral0)
            .expect("Reserve + collateral overflow")
            .saturating_sub(final_pair.total_debt0);
        assert!(
            final_vault0_balance >= required0,
            "Token0 vault balance must be >= reserve0 + collateral0 - debt0"
        );

        let required1 = final_pair
            .reserve1
            .checked_add(final_pair.total_collateral1)
            .expect("Reserve + collateral overflow")
            .saturating_sub(final_pair.total_debt1);
        assert!(
            final_vault1_balance >= required1,
            "Token1 vault balance must be >= reserve1 + collateral1 - debt1"
        );

        // INVARIANT 4: Pair state should not have fundamentally changed
        // (reserves may have changed due to update() interest accrual, but core structure remains)
        assert_eq!(
            final_pair.token0, initial_pair.token0,
            "Pair token0 should not change"
        );
        assert_eq!(
            final_pair.token1, initial_pair.token1,
            "Pair token1 should not change"
        );
        assert_eq!(
            final_pair.lp_mint, initial_pair.lp_mint,
            "Pair LP mint should not change"
        );

        // INVARIANT 5: Collateral and debt shares should not have changed
        // (flashloans shouldn't affect lending positions)
        // Note: total_debt can increase due to interest accrual, but debt_shares should remain constant
        assert_eq!(
            final_pair.total_collateral0, initial_pair.total_collateral0,
            "Total collateral0 should not change during flashloan"
        );
        assert_eq!(
            final_pair.total_collateral1, initial_pair.total_collateral1,
            "Total collateral1 should not change during flashloan"
        );
        assert!(
            final_pair.total_debt0 >= initial_pair.total_debt0,
            "Total debt0 should be at least initial (interest can accrue). Was {}, initial {}",
            final_pair.total_debt0,
            initial_pair.total_debt0
        );
        assert!(
            final_pair.total_debt1 >= initial_pair.total_debt1,
            "Total debt1 should be at least initial (interest can accrue). Was {}, initial {}",
            final_pair.total_debt1,
            initial_pair.total_debt1
        );
        assert_eq!(
            final_pair.total_debt0_shares, initial_pair.total_debt0_shares,
            "Total debt0 shares should not change during flashloan"
        );
        assert_eq!(
            final_pair.total_debt1_shares, initial_pair.total_debt1_shares,
            "Total debt1 shares should not change during flashloan"
        );

        // INVARIANT 6: Total supply should not have changed
        // (flashloans don't mint or burn LP tokens)
        assert_eq!(
            final_pair.total_supply, initial_pair.total_supply,
            "Total supply should not change during flashloan"
        );
    }
}
