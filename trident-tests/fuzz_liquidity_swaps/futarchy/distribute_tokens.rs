use crate::types::{
    omnipair::{
        DistributeTokensInstruction, DistributeTokensInstructionAccounts,
        DistributeTokensInstructionData,
    },
    DistributeTokensArgs,
};
use crate::utils::WSOL_MINT_ADDRESS;
use crate::FuzzTest;

impl FuzzTest {
    pub fn distribute_tokens(&mut self) {
        let accounts = self.get_accounts_distribute_tokens();
        let data = self.get_data_distribute_tokens();

        let pre_src_acc = self.trident.get_token_account(accounts.source_token_account);
        let pre_amount = pre_src_acc.unwrap().account.amount;

        let ix = DistributeTokensInstruction::data(DistributeTokensInstructionData::new(data))
            .accounts(accounts.clone())
            .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Distribute Tokens"));

        // Verify the transaction was successful
        assert!(res.is_success());

        // INVARIANT 1: if the source had a positive balance,
        // the distribution should drain the entire source (no dust left behind).
        if pre_amount > 0 {
            let post_src_acc = self.trident.get_token_account(accounts.source_token_account);
            let post_amount = post_src_acc.unwrap().account.amount;
            assert!(post_amount == 0, "DistributeTokens left dust in source token account (amount={})", post_amount);
        }
    }

    fn get_data_distribute_tokens(&mut self) -> DistributeTokensArgs {
        DistributeTokensArgs::new()
    }

    fn get_accounts_distribute_tokens(&mut self) -> DistributeTokensInstructionAccounts {
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        // Get the source token account - this should be an associated token account
        // that holds tokens to be distributed
        let source_token_account = self
            .fuzz_accounts
            .authority_wsol_account
            .get(&mut self.trident).expect("Authority WSOl account should exist");

        let futarchy_treasury_token_account = self
            .fuzz_accounts
            .futarchy_treasury_token_account
            .get(&mut self.trident).expect("Futarchy treasury token account should exist");

        let buybacks_vault_token_account = self
            .fuzz_accounts
            .buybacks_vault_token_account
            .get(&mut self.trident).expect("Buybacks vault token account should exist");

        let team_treasury_token_account = self
            .fuzz_accounts
            .team_treasury_token_account
            .get(&mut self.trident).expect("Team treasury token account should exist");

        DistributeTokensInstructionAccounts::new(
            futarchy_authority,
            WSOL_MINT_ADDRESS,
            source_token_account,
            futarchy_treasury_token_account,
            buybacks_vault_token_account,
            team_treasury_token_account,
        )
    }
}
