
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xf8c69e91e17587c8")]
pub struct Swap{
    pub args: SwapArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SwapInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub token_in_vault: solana_pubkey::Pubkey,
    pub token_out_vault: solana_pubkey::Pubkey,
    pub user_token_in_account: solana_pubkey::Pubkey,
    pub user_token_out_account: solana_pubkey::Pubkey,
    pub authority_token_in_account: solana_pubkey::Pubkey,
    pub token_in_mint: solana_pubkey::Pubkey,
    pub token_out_mint: solana_pubkey::Pubkey,
    pub user: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Swap {
    type ArrangedAccounts = SwapInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let token_in_vault = next_account(&mut iter)?;
        let token_out_vault = next_account(&mut iter)?;
        let user_token_in_account = next_account(&mut iter)?;
        let user_token_out_account = next_account(&mut iter)?;
        let authority_token_in_account = next_account(&mut iter)?;
        let token_in_mint = next_account(&mut iter)?;
        let token_out_mint = next_account(&mut iter)?;
        let user = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(SwapInstructionAccounts {
            pair,
            rate_model,
            futarchy_authority,
            token_in_vault,
            token_out_vault,
            user_token_in_account,
            user_token_out_account,
            authority_token_in_account,
            token_in_mint,
            token_out_mint,
            user,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}