use crate::{
    types::{
        omnipair::{
            self, InitializeInstruction, InitializeInstructionAccounts, InitializeInstructionData,
        },
        InitializeAndBootstrapArgs, Pair,
    },
    utils::{
        EVENT_AUTHORITY_ADDRESS, METADATA_SEED_PREFIX,
        MPL_TOKEN_METADATA_ID, PAIR_SEED_PREFIX, TOKEN_PROGRAM,
    },
    FuzzTest,
};
use trident_fuzz::fuzzing::{solana_sdk::rent::Rent, *};

impl FuzzTest {
    pub fn init_pair(&mut self) {
        // Init pair and bootstrap
        let data = self.get_data_init_pair();
        let accounts = self.get_accounts_init_pair(data.pair_nonce);

        let ix = InitializeInstruction::data(InitializeInstructionData::new(data.clone()))
            .accounts(accounts.clone())
            .instruction();

        // Store initial balances
        let initial_deployer_token0 = self
            .trident
            .get_token_account(accounts.deployer_token0_account)
            .expect("Deployer token0 account should exist")
            .account
            .amount;
        let initial_deployer_token1 = self
            .trident
            .get_token_account(accounts.deployer_token1_account)
            .expect("Deployer token1 account should exist")
            .account
            .amount;
        let initial_deployer_sol = self.trident.get_account(&accounts.deployer).lamports();
        let initial_authority_wsol = self
            .trident
            .get_token_account(accounts.authority_wsol_account)
            .expect("Authority WSOL token account should exist")
            .account
            .amount;

        let res = self.trident.process_transaction(&[ix], Some("Init Pair"));

        if res.is_success() {
            // Initialization must NOT succeed when token0_mint == token1_mint
            assert_ne!(
                accounts.token0_mint, accounts.token1_mint,
                "initialize succeeded with identical mints: token0_mint == token1_mint ({})",
                accounts.token0_mint
            );

            self.store_accounts_init_pair(&accounts);
            self.verify_init_pair_invariants(
                &data,
                &accounts,
                initial_deployer_token0,
                initial_deployer_token1,
                initial_deployer_sol,
                initial_authority_wsol,
            );
        }
    }

    fn get_data_init_pair(&mut self) -> InitializeAndBootstrapArgs {
        let swap_fee_bps = self.trident.random_from_range(0..=10_000); // 0 to 100%
        let half_life = self.trident.random_from_range(60..=12 * 60 * 60); // 1min to 12 hours
        let fixed_cf_bps = self.trident.random_from_range(100..=10_000); // 1% to 100%

        self.trident
            .record_histogram("INIT_PAIR_FIXED_CF_BPS", fixed_cf_bps as f64);
        self.trident
            .record_histogram("INIT_PAIR_HALF_LIFE", half_life as f64);
        self.trident
            .record_histogram("INIT_PAIR_SWAP_FEE_BPS", swap_fee_bps as f64);

        let fixed_cf_bps = if self.trident.random_from_range(0..=1) == 1 {
            self.trident
                .record_histogram("INIT_PAIR_FIXED_CF_BPS", fixed_cf_bps as f64);
            Some(fixed_cf_bps)
        } else {
            None
        };

        let mut pair_nonce = [0u8; 16];
        self.trident.random_bytes(&mut pair_nonce);

        let amount0_in = self.trident.random_from_range(100..=100_000_000_000);
        let amount1_in = self.trident.random_from_range(100..=100_000_000_000);

        self.trident
            .record_histogram("INIT_PAIR_AMOUNT0_IN", amount0_in as f64);
        self.trident
            .record_histogram("INIT_PAIR_AMOUNT1_IN", amount1_in as f64);

        // Calculate expected liquidity using the same formula as the program
        // liquidity = sqrt(amount0_in * amount1_in) - MIN_LIQUIDITY
        let expected_liquidity = (amount0_in as u128)
            .checked_mul(amount1_in as u128)
            .map(|x| x.isqrt())
            .and_then(|x| x.checked_sub(1000)) // MIN_LIQUIDITY = 1000
            .unwrap_or(0) as u64;

        // Strategy: Test different scenarios
        let min_liquidity_out = match self.trident.random_from_range(0..=100) {
            // 70% - Normal case: small slippage tolerance (0-2%)
            0..=69 => {
                let slippage_bps = self.trident.random_from_range(0..=200);
                expected_liquidity.saturating_mul(10000 - slippage_bps) / 10000
            }

            // 15% - Tight slippage: very small tolerance (0-0.5%)
            70..=84 => {
                let slippage_bps = self.trident.random_from_range(0..=50);
                expected_liquidity.saturating_mul(10000 - slippage_bps) / 10000
            }

            // 10% - Large slippage: stress test (up to 50%)
            85..=94 => {
                let slippage_bps = self.trident.random_from_range(0..=5000);
                expected_liquidity.saturating_mul(10000 - slippage_bps) / 10000
            }

            // 5% - Edge cases (should sometimes fail)
            _ => {
                match self.trident.random_from_range(0..=4) {
                    0 => 0,                                            // No slippage protection
                    1 => expected_liquidity + 1, // Impossible (expects more than possible)
                    2 => u64::MAX,               // Maximum value
                    3 => expected_liquidity,     // Exact match
                    _ => self.trident.random_from_range(1..=u64::MAX), // Random chaos
                }
            }
        };

        self.trident
            .record_histogram("INIT_PAIR_MIN_LIQUIDITY_OUT", min_liquidity_out as f64);

        let lp_name = self.trident.random_string(10);
        let lp_symbol = self.trident.random_string(10);
        let lp_uri = self.trident.random_string(10);

        InitializeAndBootstrapArgs {
            swap_fee_bps,
            half_life,
            fixed_cf_bps,
            pair_nonce,
            amount0_in,
            amount1_in,
            min_liquidity_out,
            lp_name,
            lp_symbol,
            lp_uri: "https://".to_owned() + &lp_uri,
        }
    }

    fn get_accounts_init_pair(&mut self, pair_nonce: [u8; 16]) -> InitializeInstructionAccounts {
        let token0_mint = self.fuzz_accounts.token_mint.get(&mut self.trident).expect("Token0 mint should exist");
        let mut token1_mint = self.fuzz_accounts.token_mint.get(&mut self.trident).expect("Token1 mint should exist");

        while token0_mint == token1_mint {
            token1_mint = self.fuzz_accounts.token_mint.get(&mut self.trident).expect("Token1 mint should exist");
        }

        // AKA deployer -> deploys pair
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");

        let pair = self
            .trident
            .find_program_address(
                &[
                    PAIR_SEED_PREFIX,
                    token0_mint.as_ref(),
                    token1_mint.as_ref(),
                    pair_nonce.as_ref(),
                ],
                &omnipair::program_id(),
            )
            .0;

        // futarchy authority PDA
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // rate model
        let rate_model = self.trident.random_keypair().pubkey();

        // lp mint

        let lp_mint = self.trident.random_keypair().pubkey();
        let rent = self.trident.get_sysvar::<Rent>();
        let account_custom = AccountSharedData::new(rent.minimum_balance(82), 82, &TOKEN_PROGRAM);
        self.trident.set_account_custom(&lp_mint, &account_custom);

        // lp token metadata
        let lp_token_metadata = self
            .trident
            .find_program_address(
                &[
                    METADATA_SEED_PREFIX,
                    MPL_TOKEN_METADATA_ID.as_ref(),
                    lp_mint.as_ref(),
                ],
                &MPL_TOKEN_METADATA_ID,
            )
            .0;

        // deployer lp token account
        let deployer_lp_token_account =
            self.trident
                .get_associated_token_address(&lp_mint, &user, &TOKEN_PROGRAM);

        // token0 vault
        let token0_vault =
            self.trident
                .get_associated_token_address(&token0_mint, &pair, &TOKEN_PROGRAM);

        // token1 vault
        let token1_vault =
            self.trident
                .get_associated_token_address(&token1_mint, &pair, &TOKEN_PROGRAM);

        // deployer token0 account
        let deployer_token0_account =
            self.trident
                .get_associated_token_address(&token0_mint, &user, &TOKEN_PROGRAM);

        // deployer token1 account
        let deployer_token1_account =
            self.trident
                .get_associated_token_address(&token1_mint, &user, &TOKEN_PROGRAM);

        InitializeInstructionAccounts::new(
            user,
            token0_mint,
            token1_mint,
            pair,
            futarchy_authority,
            rate_model,
            lp_mint,
            lp_token_metadata,
            deployer_lp_token_account,
            token0_vault,
            token1_vault,
            deployer_token0_account,
            deployer_token1_account,
            self.fuzz_accounts
                .authority_wsol_account
                .get(&mut self.trident).expect("Authority WSOL account should exist"),
            EVENT_AUTHORITY_ADDRESS,
            omnipair::program_id(),
        )
    }

    fn store_accounts_init_pair(&mut self, accounts: &InitializeInstructionAccounts) {
        self.fuzz_accounts.pair.insert_with_address(accounts.pair);
        self.fuzz_accounts
            .rate_model
            .insert_with_address(accounts.rate_model);
        self.fuzz_accounts
            .lp_mint
            .insert_with_address(accounts.lp_mint);
    }

    fn verify_init_pair_invariants(
        &mut self,
        args: &InitializeAndBootstrapArgs,
        accounts: &InitializeInstructionAccounts,
        _initial_deployer_token0: u64,
        _initial_deployer_token1: u64,
        _initial_deployer_sol: u64,
        initial_authority_wsol: u64,
    ) {
        const PAIR_CREATION_FEE_LAMPORTS: u64 = 200_000_000; // 0.2 SOL
        const MIN_LIQUIDITY: u64 = 1000;

        // Calculate expected liquidity using the same formula as the program
        // liquidity = sqrt(amount0_in * amount1_in) - MIN_LIQUIDITY
        let expected_liquidity = (args.amount0_in as u128)
            .checked_mul(args.amount1_in as u128)
            .map(|x| x.isqrt())
            .and_then(|x| x.checked_sub(MIN_LIQUIDITY as u128))
            .unwrap_or(0) as u64;

        // Check pair state (fresh for each new pair)
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .expect("Pair account should exist");

        // Check pair reserves match amounts deposited
        assert_eq!(
            pair_account.reserve0, args.amount0_in,
            "Pair reserve0 should match amount0_in"
        );
        assert_eq!(
            pair_account.reserve1, args.amount1_in,
            "Pair reserve1 should match amount1_in"
        );

        // Check pair total_supply includes MIN_LIQUIDITY (locked/burned permanently)
        assert_eq!(
            pair_account.total_supply,
            expected_liquidity + MIN_LIQUIDITY,
            "Pair total_supply should match total liquidity (including locked MIN_LIQUIDITY)"
        );

        // Check pair parameters
        assert_eq!(
            pair_account.swap_fee_bps, args.swap_fee_bps,
            "Pair swap_fee_bps should match args"
        );
        assert_eq!(
            pair_account.half_life, args.half_life,
            "Pair half_life should match args"
        );
        assert_eq!(
            pair_account.fixed_cf_bps, args.fixed_cf_bps,
            "Pair fixed_cf_bps should match args"
        );

        // Check vault balances (vaults might be reused in fuzzing, so check >= pair reserves)
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

        // Critical accounting invariant: vault balances must be at least pair reserves
        assert!(
            vault0_balance >= pair_account.reserve0,
            "Token0 vault balance must be >= pair reserve0"
        );
        assert!(
            vault1_balance >= pair_account.reserve1,
            "Token1 vault balance must be >= pair reserve1"
        );

        // Check authority WSOL balance increased by pair creation fee
        let final_authority_wsol = self
            .trident
            .get_token_account(accounts.authority_wsol_account)
            .expect("Authority WSOL account should exist")
            .account
            .amount;
        assert!(
            final_authority_wsol >= initial_authority_wsol + PAIR_CREATION_FEE_LAMPORTS,
            "Authority WSOL balance should increase by at least pair creation fee"
        );
    }
}
