
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x69458234c41cb078")]
pub struct DistributeTokens{
    pub args: DistributeTokensArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct DistributeTokensInstructionAccounts {
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub source_mint: solana_pubkey::Pubkey,
    pub source_token_account: solana_pubkey::Pubkey,
    pub futarchy_treasury_token_account: solana_pubkey::Pubkey,
    pub buybacks_vault_token_account: solana_pubkey::Pubkey,
    pub team_treasury_token_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for DistributeTokens {
    type ArrangedAccounts = DistributeTokensInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let futarchy_authority = next_account(&mut iter)?;
        let source_mint = next_account(&mut iter)?;
        let source_token_account = next_account(&mut iter)?;
        let futarchy_treasury_token_account = next_account(&mut iter)?;
        let buybacks_vault_token_account = next_account(&mut iter)?;
        let team_treasury_token_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;

        Some(DistributeTokensInstructionAccounts {
            futarchy_authority,
            source_mint,
            source_token_account,
            futarchy_treasury_token_account,
            buybacks_vault_token_account,
            team_treasury_token_account,
            token_program,
        })
    }
}