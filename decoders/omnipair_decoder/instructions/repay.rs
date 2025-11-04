
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xea674352d0eadba6")]
pub struct Repay{
    pub args: AdjustPositionArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct RepayInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub user_position: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub token_vault: solana_pubkey::Pubkey,
    pub user_token_account: solana_pubkey::Pubkey,
    pub vault_token_mint: solana_pubkey::Pubkey,
    pub user: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Repay {
    type ArrangedAccounts = RepayInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let user_position = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let token_vault = next_account(&mut iter)?;
        let user_token_account = next_account(&mut iter)?;
        let vault_token_mint = next_account(&mut iter)?;
        let user = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(RepayInstructionAccounts {
            pair,
            user_position,
            rate_model,
            futarchy_authority,
            token_vault,
            user_token_account,
            vault_token_mint,
            user,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}