use crate::utils::{WSOL_MINT_ADDRESS};
use crate::FuzzTest;
use crate::{
    types::{
        omnipair::{
            InitFutarchyAuthorityInstruction, InitFutarchyAuthorityInstructionAccounts,
            InitFutarchyAuthorityInstructionData,
        },
        FutarchyAuthority, InitFutarchyAuthorityArgs,
    },
    utils::DEPLOYER_ADDRESS,
};
use trident_fuzz::fuzzing::*;

impl FuzzTest {
    pub fn init_futarchy(&mut self) {
        // Init futarchy authority
        let data = self.get_data_init_futarchy();
        let accounts = self.get_accounts_init_futarchy();

        let ix = InitFutarchyAuthorityInstruction::data(InitFutarchyAuthorityInstructionData::new(
            data.clone(),
        ))
        .accounts(accounts.clone())
        .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Init Futarchy Authority"));

        // MUST BE SUCCESSFUL
        assert!(res.is_success());

        // Verify invariants
        self.verify_futarchy_invariants(&data, &accounts);
    }

    fn get_data_init_futarchy(&mut self) -> InitFutarchyAuthorityArgs {
        let swap_bps = self.trident.random_from_range(1..=5_000);
        let interest_bps = self.trident.random_from_range(1..=5_000);

        self.trident
            .record_histogram("INIT_FUTARCHY_SWAP_BPS", swap_bps as f64);
        self.trident
            .record_histogram("INIT_FUTARCHY_INTEREST_BPS", interest_bps as f64);

        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // Futarchy treasury token account
        let futarchy_treasury_token_account = self
            .fuzz_accounts
            .futarchy_treasury_token_account
            .insert(&mut self.trident, None);
        let init_futarchy_treasury_token_account_ix = self.trident.initialize_token_account(
            &DEPLOYER_ADDRESS,
            &futarchy_treasury_token_account,
            &WSOL_MINT_ADDRESS,
            &futarchy_authority,
        );

        // Buybacks vault token account
        let buybacks_vault_token_account = self
            .fuzz_accounts
            .buybacks_vault_token_account
            .insert(&mut self.trident, None);
        let init_buybacks_vault_token_account_ix = self.trident.initialize_token_account(
            &DEPLOYER_ADDRESS,
            &buybacks_vault_token_account,
            &WSOL_MINT_ADDRESS,
            &futarchy_authority,
        );
        // Team treasury token account
        let team_treasury_token_account = self
            .fuzz_accounts
            .team_treasury_token_account
            .insert(&mut self.trident, None);
        let init_team_treasury_token_account_ix = self.trident.initialize_token_account(
            &DEPLOYER_ADDRESS,
            &team_treasury_token_account,
            &WSOL_MINT_ADDRESS,
            &futarchy_authority,
        );
        let ixs: Vec<Instruction> = vec![
            init_futarchy_treasury_token_account_ix,
            init_buybacks_vault_token_account_ix,
            init_team_treasury_token_account_ix,
        ]
        .into_iter()
        .flatten()
        .collect();
        let res = self.trident.process_transaction(&ixs, None);
        assert!(res.is_success());

        // Generate random bps values that sum to exactly 10_000
        let futarchy_treasury_bps = self.trident.random_from_range(0..=10_000);
        let remaining_after_first = 10_000 - futarchy_treasury_bps;

        let buybacks_vault_bps = if remaining_after_first > 0 {
            self.trident.random_from_range(0..=remaining_after_first)
        } else {
            0
        };

        // Calculate the remaining to ensure sum equals 10_000
        let team_treasury_bps = 10_000u16
            .saturating_sub(futarchy_treasury_bps)
            .saturating_sub(buybacks_vault_bps);

        self.trident.record_histogram(
            "INIT_FUTARCHY_FUTARCHY_TREASURY_BPS",
            futarchy_treasury_bps as f64,
        );
        self.trident.record_histogram(
            "INIT_FUTARCHY_BUYBACKS_VAULT_BPS",
            buybacks_vault_bps as f64,
        );
        self.trident
            .record_histogram("INIT_FUTARCHY_TEAM_TREASURY_BPS", team_treasury_bps as f64);

        InitFutarchyAuthorityArgs {
            authority: DEPLOYER_ADDRESS,
            swap_bps,
            interest_bps,
            futarchy_treasury: futarchy_treasury_token_account,
            futarchy_treasury_bps,
            buybacks_vault: buybacks_vault_token_account,
            buybacks_vault_bps,
            team_treasury: team_treasury_token_account,
            team_treasury_bps,
        }
    }

    fn get_accounts_init_futarchy(&mut self) -> InitFutarchyAuthorityInstructionAccounts {
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        InitFutarchyAuthorityInstructionAccounts::new(futarchy_authority)
    }

    fn verify_futarchy_invariants(
        &mut self,
        data: &InitFutarchyAuthorityArgs,
        accounts: &InitFutarchyAuthorityInstructionAccounts,
    ) {
        let futarchy_authority_account = self
            .trident
            .get_account_with_type::<FutarchyAuthority>(&accounts.futarchy_authority, 8);

        if let Some(futarchy_authority_account) = futarchy_authority_account {
            // Verify recipients are set correctly
            assert_eq!(
                futarchy_authority_account.recipients.futarchy_treasury, data.futarchy_treasury,
                "Futarchy treasury recipient mismatch"
            );
            assert_eq!(
                futarchy_authority_account.recipients.buybacks_vault, data.buybacks_vault,
                "Buybacks vault recipient mismatch"
            );
            assert_eq!(
                futarchy_authority_account.recipients.team_treasury, data.team_treasury,
                "Team treasury recipient mismatch"
            );

            // Verify revenue distribution BPS sum to 10_000
            let total_bps = futarchy_authority_account
                .revenue_distribution
                .futarchy_treasury_bps
                .saturating_add(
                    futarchy_authority_account
                        .revenue_distribution
                        .buybacks_vault_bps,
                )
                .saturating_add(
                    futarchy_authority_account
                        .revenue_distribution
                        .team_treasury_bps,
                );
            assert_eq!(
                total_bps, 10_000,
                "Revenue distribution BPS must sum to 10_000"
            );

            // Verify revenue share BPS are within valid range
            assert!(
                futarchy_authority_account.revenue_share.swap_bps <= 10_000,
                "Swap BPS must be <= 10_000"
            );
            assert!(
                futarchy_authority_account.revenue_share.interest_bps <= 10_000,
                "Interest BPS must be <= 10_000"
            );
        }
    }
}
