use trident_fuzz::fuzzing::{Pubkey};

use crate::{
    types::{
        omnipair::{
            self, RemoveLiquidityInstruction, RemoveLiquidityInstructionAccounts,
            RemoveLiquidityInstructionData,
        },
        Pair, RemoveLiquidityArgs,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn remove_liquidity(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            // No pairs found, skip
            return;
        }

        let accounts = self.get_accounts_remove_liquidity();

        // Check if user has any LP token account initialized
        if self
            .trident
            .get_token_account(accounts.user_lp_token_account)
            .is_err()
        {
            // No LP token account found, skip
            return;
        }

        let data = self.get_data_remove_liquidity(accounts.pair, accounts.user_lp_token_account);

        self.trident
            .record_histogram("REMOVE_LIQUIDITY_LIQUIDITY_IN", data.liquidity_in as f64);
        self.trident.record_histogram(
            "REMOVE_LIQUIDITY_MIN_AMOUNT0_OUT",
            data.min_amount0_out as f64,
        );
        self.trident.record_histogram(
            "REMOVE_LIQUIDITY_MIN_AMOUNT1_OUT",
            data.min_amount1_out as f64,
        );

        // Store initial state
        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");
        let initial_user_lp = self
            .trident
            .get_token_account(accounts.user_lp_token_account)
            .expect("User LP account should exist")
            .account
            .amount;
        let initial_user_token0 = self
            .trident
            .get_token_account(accounts.user_token0_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);
        let initial_user_token1 = self
            .trident
            .get_token_account(accounts.user_token1_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let ix =
            RemoveLiquidityInstruction::data(RemoveLiquidityInstructionData::new(data.clone()))
                .accounts(accounts.clone())
                .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Remove Liquidity"));

        if res.is_success() {
            self.verify_remove_liquidity_invariants(
                &data,
                &accounts,
                &initial_pair,
                initial_user_lp,
                initial_user_token0,
                initial_user_token1,
            );
        }
    }

    fn get_data_remove_liquidity(
        &mut self,
        pair_pubkey: Pubkey,
        user_lp_token_account: Pubkey,
    ) -> RemoveLiquidityArgs {
        let user_lp_balance = self
            .trident
            .get_token_account(user_lp_token_account)
            .expect("User LP token account should exist")
            .account
            .amount;

        // Early return if user has no LP tokens to avoid empty range errors
        if user_lp_balance == 0 {
            return RemoveLiquidityArgs {
                liquidity_in: 0,
                min_amount0_out: 0,
                min_amount1_out: 0,
            };
        }

        // First, get the current pair state
        let pair = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .expect("Pair should exist");

        let reserve0 = pair.reserve0;
        let reserve1 = pair.reserve1;
        let total_supply = pair.total_supply;

        // Strategy: Test different liquidity removal scenarios
        match self.trident.random_from_range(0..=100) {
            // 40% - Proportional removal (remove a percentage of user's LP tokens)
            0..=39 => {
                // Remove between 1% and 100% of user's LP tokens
                let percentage = self.trident.random_from_range(1..=100);
                let liquidity_in = user_lp_balance
                    .saturating_mul(percentage)
                    .checked_div(100)
                    .unwrap_or(0);

                // Calculate expected amounts
                let amount0_out = (liquidity_in as u128)
                    .checked_mul(reserve0 as u128)
                    .unwrap()
                    .checked_div(total_supply as u128)
                    .unwrap() as u64;

                let amount1_out = (liquidity_in as u128)
                    .checked_mul(reserve1 as u128)
                    .unwrap()
                    .checked_div(total_supply as u128)
                    .unwrap() as u64;

                // Set min amounts with small slippage tolerance (0-2%)
                let slippage_bps = self.trident.random_from_range(0..=200);
                let slippage_multiplier = 10000u64.checked_sub(slippage_bps).unwrap_or(10000);
                let min_amount0_out = amount0_out
                    .saturating_mul(slippage_multiplier)
                    .checked_div(10000)
                    .unwrap_or(0);
                let min_amount1_out = amount1_out
                    .saturating_mul(slippage_multiplier)
                    .checked_div(10000)
                    .unwrap_or(0);

                RemoveLiquidityArgs {
                    liquidity_in,
                    min_amount0_out,
                    min_amount1_out,
                }
            }

            // 30% - Small amounts (dust testing)
            40..=69 => {
                let liquidity_in = self
                    .trident
                    .random_from_range(1..=1000.min(user_lp_balance.max(1)));

                // Calculate expected amounts
                let amount0_out = (liquidity_in as u128)
                    .checked_mul(reserve0 as u128)
                    .unwrap_or(0)
                    .checked_div(total_supply as u128)
                    .unwrap_or(0) as u64;

                let amount1_out = (liquidity_in as u128)
                    .checked_mul(reserve1 as u128)
                    .unwrap_or(0)
                    .checked_div(total_supply as u128)
                    .unwrap_or(0) as u64;

                RemoveLiquidityArgs {
                    liquidity_in,
                    min_amount0_out: amount0_out.saturating_sub(1), // Allow for rounding
                    min_amount1_out: amount1_out.saturating_sub(1),
                }
            }

            // 15% - Remove all liquidity
            70..=84 => {
                let liquidity_in = user_lp_balance;

                // Calculate expected amounts
                let amount0_out = (liquidity_in as u128)
                    .checked_mul(reserve0 as u128)
                    .unwrap()
                    .checked_div(total_supply as u128)
                    .unwrap() as u64;

                let amount1_out = (liquidity_in as u128)
                    .checked_mul(reserve1 as u128)
                    .unwrap()
                    .checked_div(total_supply as u128)
                    .unwrap() as u64;

                // Allow for some slippage
                let slippage_bps = self.trident.random_from_range(0..=500);
                let slippage_multiplier = 10000u64.checked_sub(slippage_bps).unwrap_or(10000);
                let min_amount0_out = amount0_out
                    .saturating_mul(slippage_multiplier)
                    .checked_div(10000)
                    .unwrap_or(0);
                let min_amount1_out = amount1_out
                    .saturating_mul(slippage_multiplier)
                    .checked_div(10000)
                    .unwrap_or(0);

                RemoveLiquidityArgs {
                    liquidity_in,
                    min_amount0_out,
                    min_amount1_out,
                }
            }

            // 10% - Unrealistic expectations (should fail)
            85..=94 => {
                let liquidity_in = self
                    .trident
                    .random_from_range(1..=user_lp_balance.max(1000));

                // Calculate expected amounts
                let amount0_out = (liquidity_in as u128)
                    .checked_mul(reserve0 as u128)
                    .unwrap_or(0)
                    .checked_div(total_supply as u128)
                    .unwrap_or(0) as u64;

                let amount1_out = (liquidity_in as u128)
                    .checked_mul(reserve1 as u128)
                    .unwrap_or(0)
                    .checked_div(total_supply as u128)
                    .unwrap_or(0) as u64;

                // Set unrealistic min amounts (should fail with InsufficientOutput)
                let multiplier = self.trident.random_from_range(2..=10);
                RemoveLiquidityArgs {
                    liquidity_in,
                    min_amount0_out: amount0_out.saturating_mul(multiplier),
                    min_amount1_out: amount1_out.saturating_mul(multiplier),
                }
            }

            // 5% - Edge cases (should fail)
            _ => {
                match self.trident.random_from_range(0..=4) {
                    // Zero liquidity (should fail with AmountZero)
                    0 => RemoveLiquidityArgs {
                        liquidity_in: 0,
                        min_amount0_out: 0,
                        min_amount1_out: 0,
                    },
                    // More liquidity than user has (should fail with InsufficientBalance)
                    1 => RemoveLiquidityArgs {
                        liquidity_in: user_lp_balance.saturating_add(1_000_000),
                        min_amount0_out: 0,
                        min_amount1_out: 0,
                    },
                    // Unrealistic min amounts
                    2 => RemoveLiquidityArgs {
                        liquidity_in: self.trident.random_from_range(1..=1000),
                        min_amount0_out: u64::MAX,
                        min_amount1_out: u64::MAX,
                    },
                    // Overflow scenarios
                    3 => RemoveLiquidityArgs {
                        liquidity_in: u64::MAX,
                        min_amount0_out: 0,
                        min_amount1_out: 0,
                    },
                    // Random chaos
                    _ => RemoveLiquidityArgs {
                        liquidity_in: self.trident.random_from_range(1..=u64::MAX),
                        min_amount0_out: self.trident.random_from_range(0..=u64::MAX),
                        min_amount1_out: self.trident.random_from_range(0..=u64::MAX),
                    },
                }
            }
        }
    }

    fn get_accounts_remove_liquidity(&mut self) -> RemoveLiquidityInstructionAccounts {
        let pair_pubkey = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .expect("Pair should exist");

        let futarchy_authority_pubkey =
            self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

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

        // user (reuse existing user or create new one)
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        // user token0 account
        let user_token0_account =
            self.trident
                .get_associated_token_address(&pair_account.token0, &user, &TOKEN_PROGRAM);

        // user token1 account
        let user_token1_account =
            self.trident
                .get_associated_token_address(&pair_account.token1, &user, &TOKEN_PROGRAM);

        // user lp token account
        let user_lp_token_account =
            self.trident
                .get_associated_token_address(&pair_account.lp_mint, &user, &TOKEN_PROGRAM);

        RemoveLiquidityInstructionAccounts::new(
            pair_pubkey,
            pair_account.rate_model,
            futarchy_authority_pubkey,
            token0_vault,
            token1_vault,
            user_token0_account,
            user_token1_account,
            pair_account.token0,
            pair_account.token1,
            pair_account.lp_mint,
            user_lp_token_account,
            user,
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_remove_liquidity_invariants(
        &mut self,
        args: &RemoveLiquidityArgs,
        accounts: &RemoveLiquidityInstructionAccounts,
        initial_pair: &Pair,
        initial_user_lp: u64,
        initial_user_token0: u64,
        initial_user_token1: u64,
    ) {
        // Get final pair state
        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist after remove liquidity");

        // Get actual user balances to determine what they received
        let final_user_token0 = self
            .trident
            .get_token_account(accounts.user_token0_account)
            .expect("User token0 account should exist")
            .account
            .amount;
        let final_user_token1 = self
            .trident
            .get_token_account(accounts.user_token1_account)
            .expect("User token1 account should exist")
            .account
            .amount;

        // Calculate actual amounts received by user
        let actual_amount0_received = final_user_token0.checked_sub(initial_user_token0).unwrap();
        let actual_amount1_received = final_user_token1.checked_sub(initial_user_token1).unwrap();

        // Check amounts received meet minimum requirements (slippage protection)
        assert!(
            actual_amount0_received >= args.min_amount0_out,
            "User should receive at least min_amount0_out"
        );
        assert!(
            actual_amount1_received >= args.min_amount1_out,
            "User should receive at least min_amount1_out"
        );

        // NOTE: We cannot directly check reserve deltas because pair.update() accrues interest
        // during the transaction, which adds to reserves. This makes the net reserve decrease
        // smaller than what the user received. Instead, we verify:
        // 1. User received what they expected (checked above)
        // 2. LP tokens were burned correctly (checked below)
        // 3. Vault solvency is maintained (checked below)

        // Check pair total_supply decreased by liquidity burned
        let expected_total_supply = initial_pair
            .total_supply
            .checked_sub(args.liquidity_in)
            .unwrap();
        assert_eq!(
            final_pair.total_supply, expected_total_supply,
            "Pair total_supply should decrease by liquidity_in"
        );

        // Check user LP balance decreased by liquidity_in
        let final_user_lp = self
            .trident
            .get_token_account(accounts.user_lp_token_account)
            .expect("User LP account should exist")
            .account
            .amount;
        let expected_user_lp = initial_user_lp.checked_sub(args.liquidity_in).unwrap();
        assert_eq!(
            final_user_lp, expected_user_lp,
            "User LP balance should decrease by liquidity_in"
        );

        // Critical accounting invariant: vault balances must be >= pair reserves + collateral
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

        // Vaults must have enough for reserves + collateral combined
        let required_vault0 = final_pair
            .reserve0
            .checked_add(final_pair.total_collateral0)
            .expect("Overflow calculating required vault0");
        let required_vault1 = final_pair
            .reserve1
            .checked_add(final_pair.total_collateral1)
            .expect("Overflow calculating required vault1");

        assert!(
            vault0_balance >= required_vault0,
            "Token0 vault balance must be >= reserve0 + total_collateral0. Vault: {}, Required: {}",
            vault0_balance,
            required_vault0
        );
        assert!(
            vault1_balance >= required_vault1,
            "Token1 vault balance must be >= reserve1 + total_collateral1. Vault: {}, Required: {}",
            vault1_balance,
            required_vault1
        );
    }
}
