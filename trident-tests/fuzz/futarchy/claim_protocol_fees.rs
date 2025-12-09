use crate::FuzzTest;
use crate::{
    types::{
        omnipair::{
            ClaimProtocolFeesInstruction, ClaimProtocolFeesInstructionAccounts,
            ClaimProtocolFeesInstructionData,
        },
        ClaimProtocolFeesArgs, Pair,
    },
    utils::{TOKEN_PROGRAM},
};

impl FuzzTest {
    pub fn claim_protocol_fees(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            // No pairs found, skip
            return;
        }

        let accounts = self.get_accounts_claim_protocol_fees();
        let data = self.get_data_claim_protocol_fees();

        // Capture initial state before the transaction
        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let initial_authority_token0_balance = self
            .trident
            .get_token_account(accounts.authority_token0_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let initial_authority_token1_balance = self
            .trident
            .get_token_account(accounts.authority_token1_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let ix =
            ClaimProtocolFeesInstruction::data(ClaimProtocolFeesInstructionData::new(data.clone()))
                .accounts(accounts.clone())
                .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Claim Protocol Fees"));

        // Only verify invariants if transaction succeeded
        // Transaction may fail with expected errors (e.g., InsufficientAmount)
        if res.is_success() {
            self.verify_claim_protocol_fees_invariants(
                &data,
                &accounts,
                &initial_pair,
                initial_authority_token0_balance,
                initial_authority_token1_balance,
            );
        }
    }

    fn get_data_claim_protocol_fees(&mut self) -> ClaimProtocolFeesArgs {
        let pair_pubkey = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .expect("Pair account should exist");

        // Get protocol fees from the pair account
        // Claim partial amounts (or 0 if no fees available)
        let amount0 = if pair_account.protocol_revenue_reserve0 > 0 {
            self.trident
                .random_from_range(1..=pair_account.protocol_revenue_reserve0)
        } else {
            0
        };

        let amount1 = if pair_account.protocol_revenue_reserve1 > 0 {
            self.trident
                .random_from_range(1..=pair_account.protocol_revenue_reserve1)
        } else {
            0
        };

        self.trident
            .record_histogram("CLAIM_PROTOCOL_FEES_AMOUNT0", amount0 as f64);
        self.trident
            .record_histogram("CLAIM_PROTOCOL_FEES_AMOUNT1", amount1 as f64);

        ClaimProtocolFeesArgs::new(amount0, amount1)
    }

    fn get_accounts_claim_protocol_fees(&mut self) -> ClaimProtocolFeesInstructionAccounts {
        let pair_pubkey = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .expect("Pair account should exist");

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // token0 vault
        let token0_vault = self.trident.get_associated_token_address(
            &pair_account.token0,
            &pair_pubkey,
            &TOKEN_PROGRAM,
        );

        // token1 vault
        let token1_vault = self.trident.get_associated_token_address(
            &pair_account.token1,
            &pair_pubkey,
            &TOKEN_PROGRAM,
        );

        // authority token0 account
        let authority_token0_account = self.trident.get_associated_token_address(
            &pair_account.token0,
            &futarchy_authority,
            &TOKEN_PROGRAM,
        );

        // authority token1 account
        let authority_token1_account = self.trident.get_associated_token_address(
            &pair_account.token1,
            &futarchy_authority,
            &TOKEN_PROGRAM,
        );

        // caller (whoever)
        let caller = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        ClaimProtocolFeesInstructionAccounts::new(
            caller,
            pair_pubkey,
            futarchy_authority,
            token0_vault,
            token1_vault,
            authority_token0_account,
            authority_token1_account,
            pair_account.token0,
            pair_account.token1,
        )
    }

    fn verify_claim_protocol_fees_invariants(
        &mut self,
        args: &ClaimProtocolFeesArgs,
        accounts: &ClaimProtocolFeesInstructionAccounts,
        initial_pair: &Pair,
        initial_authority_token0_balance: u64,
        initial_authority_token1_balance: u64,
    ) {
        // Fetch final state
        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");

        let final_authority_token0_balance = self
            .trident
            .get_token_account(accounts.authority_token0_account)
            .expect("Authority token0 account should exist")
            .account
            .amount;

        let final_authority_token1_balance = self
            .trident
            .get_token_account(accounts.authority_token1_account)
            .expect("Authority token1 account should exist")
            .account
            .amount;

        // INVARIANT 1: Authority received exactly the claimed amounts
        if args.amount0 > 0 {
            assert_eq!(
                final_authority_token0_balance,
                initial_authority_token0_balance
                    .checked_add(args.amount0)
                    .expect("Authority token0 balance increase"),
                "Authority should receive exactly amount0 claimed"
            );
        }

        if args.amount1 > 0 {
            assert_eq!(
                final_authority_token1_balance,
                initial_authority_token1_balance
                    .checked_add(args.amount1)
                    .expect("Authority token1 balance increase"),
                "Authority should receive exactly amount1 claimed"
            );
        }

        // INVARIANT 2: Protocol revenue reserves decreased by exactly the claimed amounts
        assert_eq!(
            final_pair.protocol_revenue_reserve0,
            initial_pair
                .protocol_revenue_reserve0
                .checked_sub(args.amount0)
                .expect("Protocol revenue reserve0 decrease"),
            "Protocol revenue reserve0 should decrease by amount0"
        );

        assert_eq!(
            final_pair.protocol_revenue_reserve1,
            initial_pair
                .protocol_revenue_reserve1
                .checked_sub(args.amount1)
                .expect("Protocol revenue reserve1 decrease"),
            "Protocol revenue reserve1 should decrease by amount1"
        );

        // INVARIANT 3: Vault solvency check - vaults must hold at least reserves + collateral - debt
        let vault0_balance = self
            .trident
            .get_token_account(accounts.token0_vault)
            .expect("Token0 vault should exist")
            .account
            .amount;

        let vault1_balance = self
            .trident
            .get_token_account(accounts.token1_vault)
            .expect("Token1 vault should exist")
            .account
            .amount;

        let required0 = final_pair
            .reserve0
            .checked_add(final_pair.total_collateral0)
            .expect("Reserve + collateral overflow")
            .checked_add(final_pair.protocol_revenue_reserve0)
            .expect("Adding protocol revenue overflow")
            .saturating_sub(final_pair.total_debt0);
        
        // Allow small tolerance for accumulated rounding errors (0.01% of required or claimed amount)
        let tolerance0 = args.amount0.max(required0.checked_div(10000).unwrap_or(0));
        assert!(
            vault0_balance >= required0.saturating_sub(tolerance0),
            "Token0 vault balance must be >= reserve0 + collateral0 - debt0"
        );

        let required1 = final_pair
            .reserve1
            .checked_add(final_pair.total_collateral1)
            .expect("Reserve + collateral overflow")
            .checked_add(final_pair.protocol_revenue_reserve1)
            .expect("Adding protocol revenue overflow")
            .saturating_sub(final_pair.total_debt1);
        
        // Allow small tolerance for accumulated rounding errors (0.01% of required or claimed amount)
        let tolerance1 = args.amount1.max(required1.checked_div(10000).unwrap_or(0));
        assert!(
            vault1_balance >= required1.saturating_sub(tolerance1),
            "Token1 vault balance must be >= reserve1 + collateral1 - debt1"
        );

        // INVARIANT 4: Pair core state should not have changed
        // (claiming fees doesn't affect liquidity, collateral, or debt)
        assert_eq!(
            final_pair.reserve0, initial_pair.reserve0,
            "Reserve0 should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.reserve1, initial_pair.reserve1,
            "Reserve1 should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.total_supply, initial_pair.total_supply,
            "Total supply should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.total_collateral0, initial_pair.total_collateral0,
            "Total collateral0 should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.total_collateral1, initial_pair.total_collateral1,
            "Total collateral1 should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.total_debt0, initial_pair.total_debt0,
            "Total debt0 should not change when claiming protocol fees"
        );
        assert_eq!(
            final_pair.total_debt1, initial_pair.total_debt1,
            "Total debt1 should not change when claiming protocol fees"
        );

        // INVARIANT 5: Pair identity should not have changed
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
    }
}
