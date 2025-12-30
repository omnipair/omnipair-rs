use trident_fuzz::fuzzing::{Pubkey};

use crate::{
    types::{
        omnipair::{
            self, AddLiquidityInstruction, AddLiquidityInstructionAccounts,
            AddLiquidityInstructionData,
        },
        AddLiquidityArgs, Pair,
    },
    utils::{EVENT_AUTHORITY_ADDRESS, TOKEN_PROGRAM},
    FuzzTest,
};

impl FuzzTest {
    pub fn add_liquidity(&mut self) {
        // Check if any pairs exist before trying to add liquidity
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }

        let accounts = self.get_accounts_add_liquidity();
        let data = self.get_data_add_liquidity(accounts.pair);

        let ix = AddLiquidityInstruction::data(AddLiquidityInstructionData::new(data.clone()))
            .accounts(accounts.clone())
            .instruction();

        // Store initial state
        let initial_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist");
        let initial_user_lp = self
            .trident
            .get_token_account(accounts.user_lp_token_account)
            .map(|acc| acc.account.amount)
            .unwrap_or(0);

        let res = self
            .trident
            .process_transaction(&[ix], Some("Add Liquidity"));

        if res.is_success() {
            self.verify_add_liquidity_invariants(&data, &accounts, &initial_pair, initial_user_lp);
        }
    }

    fn get_data_add_liquidity(&mut self, pair_pubkey: Pubkey) -> AddLiquidityArgs {
        // First, get the current pair state
        let pair = self
            .trident
            .get_account_with_type::<Pair>(&pair_pubkey, 8)
            .expect("Pair should exist");

        let reserve0 = pair.reserve0;
        let reserve1 = pair.reserve1;
        let total_supply = pair.total_supply;

        // Strategy: Test different liquidity addition scenarios
        match self.trident.random_from_range(0..=100) {
            // 40% - Proportional liquidity (maintains price ratio)
            0..=39 => {
                // Add liquidity proportional to current reserves
                let proportion = self.trident.random_from_range(1..=1000); // 0.1% to 100% of reserves
                let amount0_in = reserve0.saturating_mul(proportion) / 1000;
                let amount1_in = reserve1.saturating_mul(proportion) / 1000;

                // Calculate expected liquidity: both ratios should be equal
                let expected_liquidity = (amount0_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap()
                    .checked_div(reserve0 as u128)
                    .unwrap() as u64;

                // Set min_liquidity with small slippage tolerance (0-2%)
                let slippage_bps = self.trident.random_from_range(0..=200);
                let min_liquidity_out =
                    expected_liquidity.saturating_mul(10000 - slippage_bps) / 10000;

                AddLiquidityArgs {
                    amount0_in,
                    amount1_in,
                    min_liquidity_out,
                }
            }

            // 30% - Imbalanced liquidity (one side larger)
            40..=69 => {
                // This will result in min(liquidity0, liquidity1)
                let base_amount = self.trident.random_from_range(100..=100_000_000);
                let imbalance_factor = self.trident.random_from_range(50..=200); // 0.5x to 2x

                let amount0_in: u64 = base_amount;
                let amount1_in = base_amount.saturating_mul(imbalance_factor) / 100;

                // Calculate expected (will be the minimum)
                let liquidity0 = (amount0_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap()
                    .checked_div(reserve0 as u128)
                    .unwrap();
                let liquidity1 = (amount1_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap()
                    .checked_div(reserve1 as u128)
                    .unwrap();
                let expected = liquidity0.min(liquidity1) as u64;

                // Random slippage tolerance
                let slippage_bps = self.trident.random_from_range(1..=500);
                let min_liquidity_out = expected.saturating_mul(10000 - slippage_bps) / 10000;

                AddLiquidityArgs {
                    amount0_in,
                    amount1_in,
                    min_liquidity_out,
                }
            }

            // 15% - Small amounts (dust testing)
            70..=84 => {
                let amount0_in = self.trident.random_from_range(1..=1000);
                let amount1_in = self.trident.random_from_range(1..=1000);

                // Calculate expected
                let liquidity0 = (amount0_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap()
                    .checked_div(reserve0 as u128)
                    .unwrap_or(1);
                let liquidity1 = (amount1_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap()
                    .checked_div(reserve1 as u128)
                    .unwrap_or(1);
                let expected = liquidity0.min(liquidity1) as u64;

                AddLiquidityArgs {
                    amount0_in,
                    amount1_in,
                    min_liquidity_out: expected.saturating_sub(1), // Allow for rounding
                }
            }

            // 10% - Large amounts (stress testing)
            85..=94 => {
                let amount0_in = self.trident.random_from_range(1_000_000..=u64::MAX / 1000);
                let amount1_in = self.trident.random_from_range(1_000_000..=u64::MAX / 1000);

                let liquidity0 = (amount0_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap_or(u128::MAX)
                    .checked_div(reserve0 as u128)
                    .unwrap_or(u128::MAX);
                let liquidity1 = (amount1_in as u128)
                    .checked_mul(total_supply as u128)
                    .unwrap_or(u128::MAX)
                    .checked_div(reserve1 as u128)
                    .unwrap_or(u128::MAX);
                let expected = liquidity0.min(liquidity1).min(u64::MAX as u128) as u64;

                AddLiquidityArgs {
                    amount0_in,
                    amount1_in,
                    min_liquidity_out: expected / 2, // Large slippage tolerance
                }
            }

            // 5% - Edge cases (should fail)
            _ => {
                match self.trident.random_from_range(0..=4) {
                    // Zero amounts (should fail with AmountZero)
                    0 => AddLiquidityArgs {
                        amount0_in: 0,
                        amount1_in: self.trident.random_from_range(1..=1000),
                        min_liquidity_out: 0,
                    },
                    1 => AddLiquidityArgs {
                        amount0_in: self.trident.random_from_range(1..=1000),
                        amount1_in: 0,
                        min_liquidity_out: 0,
                    },
                    // Unrealistic min_liquidity (should fail with InsufficientLiquidity)
                    2 => {
                        let amount0_in = self.trident.random_from_range(100..=10000);
                        let amount1_in = self.trident.random_from_range(100..=10000);
                        AddLiquidityArgs {
                            amount0_in,
                            amount1_in,
                            min_liquidity_out: u64::MAX, // Impossible to satisfy
                        }
                    }
                    // Overflow scenarios
                    3 => AddLiquidityArgs {
                        amount0_in: u64::MAX,
                        amount1_in: u64::MAX,
                        min_liquidity_out: 0,
                    },
                    // Random chaos
                    _ => AddLiquidityArgs {
                        amount0_in: self.trident.random_from_range(1..=u64::MAX),
                        amount1_in: self.trident.random_from_range(1..=u64::MAX),
                        min_liquidity_out: self.trident.random_from_range(1..=u64::MAX),
                    },
                }
            }
        }
    }

    fn get_accounts_add_liquidity(&mut self) -> AddLiquidityInstructionAccounts {
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

        //user token0 account
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        let user_token0_account =
            self.trident
                .get_associated_token_address(&pair_account.token0, &user, &TOKEN_PROGRAM);

        //user token1 account
        let user_token1_account =
            self.trident
                .get_associated_token_address(&pair_account.token1, &user, &TOKEN_PROGRAM);

        // user lp token account
        let user_lp_token_account =
            self.trident
                .get_associated_token_address(&pair_account.lp_mint, &user, &TOKEN_PROGRAM);

        AddLiquidityInstructionAccounts::new(
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

    fn verify_add_liquidity_invariants(
        &mut self,
        args: &AddLiquidityArgs,
        accounts: &AddLiquidityInstructionAccounts,
        initial_pair: &Pair,
        initial_user_lp: u64,
    ) {
        // Get final pair state
        let final_pair = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair should exist after add liquidity");

        // Calculate expected liquidity using the same formula as the program
        // liquidity = min(amount0_in * total_supply / reserve0, amount1_in * total_supply / reserve1)
        let liquidity0 = (args.amount0_in as u128)
            .checked_mul(initial_pair.total_supply as u128)
            .unwrap()
            .checked_div(initial_pair.reserve0 as u128)
            .unwrap();
        let liquidity1 = (args.amount1_in as u128)
            .checked_mul(initial_pair.total_supply as u128)
            .unwrap()
            .checked_div(initial_pair.reserve1 as u128)
            .unwrap();
        let expected_liquidity = liquidity0.min(liquidity1) as u64;

        // Check pair reserves increased by at least the deposited amounts
        // Note: reserves can increase more than deposited amounts due to LP share of accrued interest
        let min_expected_reserve0 = initial_pair.reserve0.checked_add(args.amount0_in).unwrap();
        let min_expected_reserve1 = initial_pair.reserve1.checked_add(args.amount1_in).unwrap();
        assert!(
            final_pair.reserve0 >= min_expected_reserve0,
            "Pair reserve0 should be at least initial + amount0_in (was {}, expected >= {})",
            final_pair.reserve0,
            min_expected_reserve0
        );
        assert!(
            final_pair.reserve1 >= min_expected_reserve1,
            "Pair reserve1 should be at least initial + amount1_in (was {}, expected >= {})",
            final_pair.reserve1,
            min_expected_reserve1
        );

        // Check pair total_supply increased by liquidity minted
        let expected_total_supply = initial_pair
            .total_supply
            .checked_add(expected_liquidity)
            .unwrap();
        assert_eq!(
            final_pair.total_supply, expected_total_supply,
            "Pair total_supply should increase by liquidity minted"
        );

        // Check user received the expected liquidity tokens
        let final_user_lp = self
            .trident
            .get_token_account(accounts.user_lp_token_account)
            .expect("User LP account should exist")
            .account
            .amount;
        let expected_user_lp = initial_user_lp.checked_add(expected_liquidity).unwrap();
        assert_eq!(
            final_user_lp, expected_user_lp,
            "User should receive expected liquidity tokens"
        );

        // Check user received at least min_liquidity_out
        let lp_received = final_user_lp.saturating_sub(initial_user_lp);
        assert!(
            lp_received >= args.min_liquidity_out,
            "User should receive at least min_liquidity_out"
        );

        // Critical accounting invariant: vault balances should be >= pair reserves
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

        assert!(
            vault0_balance >= final_pair.reserve0,
            "Token0 vault balance must be >= pair reserve0"
        );
        assert!(
            vault1_balance >= final_pair.reserve1,
            "Token1 vault balance must be >= pair reserve1"
        );
    }
}
