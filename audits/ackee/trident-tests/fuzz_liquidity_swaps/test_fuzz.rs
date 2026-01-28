use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;

use crate::{
    types::omnipair,
    utils::{DEPLOYER_ADDRESS, FUTARCHY_AUTHORITY_SEED_PREFIX, TOKEN_PROGRAM, WSOL_MINT_ADDRESS},
};
mod futarchy;
mod fuzz_accounts;
mod lending;
mod liquidity;
mod spot;
mod types;
mod utils;
mod view;

const USER_COUNT: usize = 20;
const TOKEN_MINT_COUNT: usize = 2;

#[derive(FuzzTestMethods)]
struct FuzzTest {
    /// Trident client for interacting with the Solana program
    trident: Trident,
    /// Storage for all account addresses used in fuzz testing
    fuzz_accounts: AccountAddresses,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        Self {
            trident: Trident::default(),
            fuzz_accounts: AccountAddresses::default(),
        }
    }

    #[init]
    fn start(&mut self) {
        self.setup_accounts();

        self.init_futarchy();
        self.init_pair();
        self.add_liquidity();
    }

    // Liquidity flow: add more liquidity
    #[flow(weight = 20)]
    fn liquidity_flow_add(&mut self) {
        self.add_liquidity();
    }

    // Liquidity flow: add liquidity -> remove liquidity
    #[flow(weight = 25)]
    fn liquidity_flow_remove(&mut self) {
        self.add_liquidity();
        self.trident.forward_in_time(60 * 60);
        self.remove_liquidity();
    }

    // Swap flow: perform swaps
    #[flow(weight = 35)]
    fn swap_flow(&mut self) {
        self.swap();
    }

    // Combined flow: add liquidity -> swap -> remove liquidity
    #[flow(weight = 15)]
    fn combined_flow(&mut self) {
        self.add_liquidity();
        self.trident.forward_in_time(60);
        self.swap();
        self.trident.forward_in_time(60 * 60);
        self.remove_liquidity();
    }

    // View data flow
    #[flow(weight = 5)]
    fn view_flow(&mut self) {
        self.add_collateral();
        self.borrow();
        self.view_pair_data();
        self.view_user_position_data();
    }

    #[end]
    fn end(&mut self) {}

    fn setup_accounts(&mut self) {
        // Airdrop DEPLOYER_ADDRESS for transaction fees
        self.trident.airdrop(
            &DEPLOYER_ADDRESS,
            LAMPORTS_PER_SOL
                .checked_mul(5)
                .expect("Airdrop amount overflow"),
        );

        let mut users = Vec::new();

        // Pre-create user accounts
        for _ in 0..USER_COUNT {
            let user = self.fuzz_accounts.user.insert(&mut self.trident, None);
            users.push(user);
            self.trident.airdrop(
                &user,
                LAMPORTS_PER_SOL
                    .checked_mul(1000)
                    .expect("Airdrop amount overflow"),
            );
        }

        // Pre-create futarchy authority account
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.insert(
            &mut self.trident,
            Some(PdaSeeds {
                seeds: &[FUTARCHY_AUTHORITY_SEED_PREFIX],
                program_id: omnipair::program_id(),
            }),
        );

        // Pre-create authority wsol account
        let authority_wsol_account = self.trident.get_associated_token_address(
            &WSOL_MINT_ADDRESS,
            &futarchy_authority,
            &TOKEN_PROGRAM,
        );
        let ix = self.trident.initialize_associated_token_account(
            &DEPLOYER_ADDRESS,
            &WSOL_MINT_ADDRESS,
            &futarchy_authority,
        );
        self.trident.process_transaction(&[ix], None);

        self.fuzz_accounts
            .authority_wsol_account
            .insert_with_address(authority_wsol_account);

        // Pre initialize some token mints
        for _ in 0..TOKEN_MINT_COUNT {
            let token_mint = self
                .fuzz_accounts
                .token_mint
                .insert(&mut self.trident, None);

            let mint_authority = self.fuzz_accounts.user.get(&mut self.trident).expect("Mint authority should exist");
            let ix = self.trident.initialize_mint(
                &mint_authority,
                &token_mint,
                9,
                &mint_authority,
                None,
            );
            let res = self.trident.process_transaction(&ix, None);
            assert!(res.is_success());

            // Generate token account for users
            for user in users.iter() {
                // User token account
                let initialize_user_token_account_ix = self
                    .trident
                    .initialize_associated_token_account(user, &token_mint, user);
                let user_token_account =
                    self.trident
                        .get_associated_token_address(&token_mint, user, &TOKEN_PROGRAM);
                let mint_to_user_ix = self.trident.mint_to(
                    &user_token_account,
                    &token_mint,
                    &mint_authority,
                    100_000_000_000_000_000,
                );
                let res = self.trident.process_transaction(
                    &[initialize_user_token_account_ix, mint_to_user_ix],
                    None,
                );
                assert!(res.is_success());
            }
        }
    }
}

fn main() {
    FuzzTest::fuzz(1000, 100);
}
