use trident_fuzz::fuzzing::{Pubkey};

use crate::{
    types::{
        omnipair::{self, SwapInstruction, SwapInstructionAccounts, SwapInstructionData},
        Pair, SwapArgs,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn swap(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            // No pairs found, skip
            return;
        }

        let accounts = self.get_accounts_swap();
        let data = self.get_data_swap(accounts.pair, accounts.token_in_mint);

        // record histogram
        self.trident
            .record_histogram("SWAP_AMOUNT_IN", data.amount_in as f64);
        self.trident
            .record_histogram("SWAP_MIN_AMOUNT_OUT", data.min_amount_out as f64);

        // Store initial USER state only (pair state will be modified by update() during transaction)
        let initial_user_token_in = self
            .trident
            .get_token_account(accounts.user_token_in_account)
            .expect("User token in account should exist")
            .account
            .amount;
        let initial_user_token_out = self
            .trident
            .get_token_account(accounts.user_token_out_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);
        let initial_authority_token_in = self
            .trident
            .get_token_account(accounts.authority_token_in_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let ix = SwapInstruction::data(SwapInstructionData::new(data.clone()))
            .accounts(accounts.clone())
            .instruction();

        let res = self.trident.process_transaction(&[ix], Some("Swap"));

        if res.is_success() {
            self.verify_swap_invariants(
                &data,
                &accounts,
                initial_user_token_in,
                initial_user_token_out,
                initial_authority_token_in,
            );
        }
    }

    fn get_data_swap(&mut self, pair_pubkey: Pubkey, token_in_mint: Pubkey) -> SwapArgs {
        // First, get the current pair state
        let pair = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .unwrap();

        let reserve0 = pair.reserve0;
        let reserve1 = pair.reserve1;

        // Determine which token we're swapping in and get the corresponding reserves
        let (reserve_in, reserve_out) = if token_in_mint == pair.token0 {
            (reserve0, reserve1)
        } else {
            (reserve1, reserve0)
        };

        // Early return if reserves are too low to avoid empty range errors
        if reserve_in == 0 || reserve_out == 0 {
            return SwapArgs {
                amount_in: 100,
                min_amount_out: 0,
            };
        }

        // Strategy: Test different swap scenarios
        match self.trident.random_from_range(0..=100) {
            // 40% - Small to medium swaps (0.1% to 10% of reserve)
            0..=39 => {
                let percentage = self.trident.random_from_range(1..=1000); // 0.1% to 100%
                let amount_in = reserve_in.saturating_mul(percentage) / 10000; // 0.01% to 10%
                let amount_in = amount_in.max(100); // Minimum 100 tokens

                // Calculate expected output using constant product formula
                // amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
                // With fees: amount_in_after_fee = amount_in * (10000 - fee_bps) / 10000
                let swap_fee_bps = pair.swap_fee_bps;
                let amount_in_after_fee =
                    (amount_in as u128).saturating_mul(10000 - swap_fee_bps as u128) / 10000;

                let expected_out = amount_in_after_fee
                    .saturating_mul(reserve_out as u128)
                    .checked_div((reserve_in as u128).saturating_add(amount_in_after_fee))
                    .unwrap_or(0) as u64;

                // Set min_amount_out with small slippage tolerance (0-2%)
                let slippage_bps = self.trident.random_from_range(0..=200);
                let min_amount_out = expected_out.saturating_mul(10000 - slippage_bps) / 10000;

                SwapArgs {
                    amount_in,
                    min_amount_out,
                }
            }

            // 30% - Larger swaps (10% to 50% of reserve)
            40..=69 => {
                let percentage = self.trident.random_from_range(1000..=5000); // 10% to 50%
                let amount_in = reserve_in.saturating_mul(percentage) / 10000;

                let swap_fee_bps = pair.swap_fee_bps;
                let amount_in_after_fee =
                    (amount_in as u128).saturating_mul(10000 - swap_fee_bps as u128) / 10000;

                let expected_out = amount_in_after_fee
                    .saturating_mul(reserve_out as u128)
                    .checked_div((reserve_in as u128).saturating_add(amount_in_after_fee))
                    .unwrap_or(0) as u64;

                // Larger slippage tolerance for bigger swaps
                let slippage_bps = self.trident.random_from_range(100..=1000);
                let min_amount_out = expected_out.saturating_mul(10000 - slippage_bps) / 10000;

                SwapArgs {
                    amount_in,
                    min_amount_out,
                }
            }

            // 15% - Dust amounts (very small swaps)
            70..=84 => {
                let amount_in = self.trident.random_from_range(1..=1000);

                let swap_fee_bps = pair.swap_fee_bps;
                let amount_in_after_fee =
                    (amount_in as u128).saturating_mul(10000 - swap_fee_bps as u128) / 10000;

                let expected_out = amount_in_after_fee
                    .saturating_mul(reserve_out as u128)
                    .checked_div((reserve_in as u128).saturating_add(amount_in_after_fee))
                    .unwrap_or(0) as u64;

                SwapArgs {
                    amount_in,
                    min_amount_out: expected_out.saturating_sub(1), // Allow for rounding
                }
            }

            // 10% - Unrealistic expectations (should fail)
            85..=94 => {
                let amount_in = self.trident.random_from_range(100..=100_000_000);

                let swap_fee_bps = pair.swap_fee_bps;
                let amount_in_after_fee =
                    (amount_in as u128).saturating_mul(10000 - swap_fee_bps as u128) / 10000;

                let expected_out = amount_in_after_fee
                    .saturating_mul(reserve_out as u128)
                    .checked_div((reserve_in as u128).saturating_add(amount_in_after_fee))
                    .unwrap_or(0) as u64;

                // Set unrealistic min_amount_out (should fail with InsufficientOutput)
                let multiplier = self.trident.random_from_range(2..=10);
                SwapArgs {
                    amount_in,
                    min_amount_out: expected_out.saturating_mul(multiplier),
                }
            }

            // 5% - Edge cases (should fail)
            _ => {
                match self.trident.random_from_range(0..=4) {
                    // Zero amount (should fail with AmountZero)
                    0 => SwapArgs {
                        amount_in: 0,
                        min_amount_out: 0,
                    },
                    // Unrealistic min_amount_out
                    1 => SwapArgs {
                        amount_in: self.trident.random_from_range(1..=1000),
                        min_amount_out: u64::MAX,
                    },
                    // Swap entire reserve (should fail or cause extreme slippage)
                    2 => SwapArgs {
                        amount_in: reserve_in,
                        min_amount_out: 0,
                    },
                    // Overflow scenarios
                    3 => SwapArgs {
                        amount_in: u64::MAX,
                        min_amount_out: 0,
                    },
                    // Random chaos
                    _ => SwapArgs {
                        amount_in: self.trident.random_from_range(1..=u64::MAX),
                        min_amount_out: self.trident.random_from_range(0..=u64::MAX),
                    },
                }
            }
        }
    }

    fn get_accounts_swap(&mut self) -> SwapInstructionAccounts {
        let pair_pubkey = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .unwrap();

        let futarchy_authority_pubkey =
            self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // Randomly choose swap direction: token0 -> token1 or token1 -> token0
        let is_token0_in = self.trident.random_from_range(0..=1) == 0;

        let (token_in_mint, token_out_mint) = if is_token0_in {
            (pair_account.token0, pair_account.token1)
        } else {
            (pair_account.token1, pair_account.token0)
        };

        // token vaults
        let token_in_vault =
            self.trident
                .get_associated_token_address(&token_in_mint, &pair_pubkey, &TOKEN_PROGRAM);

        let token_out_vault = self.trident.get_associated_token_address(
            &token_out_mint,
            &pair_pubkey,
            &TOKEN_PROGRAM,
        );

        // user
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        // user token account
        let user_token_in_account =
            self.trident
                .get_associated_token_address(&token_in_mint, &user, &TOKEN_PROGRAM);

        let user_token_out_account =
            self.trident
                .get_associated_token_address(&token_out_mint, &user, &TOKEN_PROGRAM);

        // authority token account (for fees)
        let authority_token_in_account = self.trident.get_associated_token_address(
            &token_in_mint,
            &futarchy_authority_pubkey,
            &TOKEN_PROGRAM,
        );

        if self
            .trident
            .get_token_account(authority_token_in_account)
            .is_err()
        {
            self.trident.initialize_associated_token_account(
                &user,
                &token_in_mint,
                &futarchy_authority_pubkey,
            );
        }

        SwapInstructionAccounts::new(
            pair_pubkey,
            pair_account.rate_model,
            futarchy_authority_pubkey,
            token_in_vault,
            token_out_vault,
            user_token_in_account,
            user_token_out_account,
            token_in_mint,
            token_out_mint,
            authority_token_in_account,
            user,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    fn verify_swap_invariants(
        &mut self,
        args: &SwapArgs,
        accounts: &SwapInstructionAccounts,
        initial_user_token_in: u64,
        initial_user_token_out: u64,
        initial_authority_token_in: u64,
    ) {
        // Get final pair state
        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist after swap");

        // Get final user balances
        let final_user_token_in = self
            .trident
            .get_token_account(accounts.user_token_in_account)
            .expect("User token in account should exist")
            .account
            .amount;
        let final_user_token_out = self
            .trident
            .get_token_account(accounts.user_token_out_account)
            .expect("User token out account should exist")
            .account
            .amount;

        // Calculate actual amounts
        let actual_amount_in = initial_user_token_in
            .checked_sub(final_user_token_in)
            .unwrap();
        let actual_amount_out = final_user_token_out
            .checked_sub(initial_user_token_out)
            .unwrap();

        // Verify user paid exactly amount_in
        assert_eq!(
            actual_amount_in, args.amount_in,
            "User should pay exactly amount_in"
        );

        // Verify user received at least min_amount_out (slippage protection)
        assert!(
            actual_amount_out >= args.min_amount_out,
            "User should receive at least min_amount_out"
        );

        // Verify constant product invariant: k should not decrease
        // Note: k may increase slightly due to fees staying in the pool
        let final_k = (final_pair.reserve0 as u128)
            .checked_mul(final_pair.reserve1 as u128)
            .unwrap();

        // We can't check k against initial because update() modifies reserves with interest
        // But we can verify k is reasonable and reserves are positive
        assert!(final_k > 0, "Constant product k must be positive");
        assert!(
            final_pair.reserve0 > 0,
            "Reserve0 must be positive after swap"
        );
        assert!(
            final_pair.reserve1 > 0,
            "Reserve1 must be positive after swap"
        );

        // Verify authority received futarchy fee (if any)
        let final_authority_token_in = self
            .trident
            .get_token_account(accounts.authority_token_in_account)
            .expect("Authority token in account should exist")
            .account
            .amount;

        let authority_fee_received = final_authority_token_in
            .checked_sub(initial_authority_token_in)
            .unwrap();

        // Authority should receive some fee (unless swap is too small or fees are 0)
        // We verify it's reasonable: futarchy fee = total_fee * revenue_share_bps / 10000
        // where total_fee = amount_in * swap_fee_bps / 10000
        if args.amount_in > 1000 && final_pair.swap_fee_bps > 0 {
            assert!(
                authority_fee_received > 0,
                "Authority should receive futarchy fee for non-trivial swaps"
            );
        }

        // Critical accounting invariant: vault balances should be >= pair reserves
        let token_in_vault_balance = self
            .trident
            .get_token_account(accounts.token_in_vault)
            .expect("Token in vault should exist")
            .account
            .amount;
        let token_out_vault_balance = self
            .trident
            .get_token_account(accounts.token_out_vault)
            .expect("Token out vault should exist")
            .account
            .amount;

        // Determine which reserve corresponds to which vault
        let is_token0_in = accounts.token_in_mint == final_pair.token0;
        let (reserve_in, reserve_out) = if is_token0_in {
            (final_pair.reserve0, final_pair.reserve1)
        } else {
            (final_pair.reserve1, final_pair.reserve0)
        };

        assert!(
            token_in_vault_balance >= reserve_in,
            "Token in vault balance must be >= corresponding reserve"
        );
        assert!(
            token_out_vault_balance >= reserve_out,
            "Token out vault balance must be >= corresponding reserve"
        );
    }
}
