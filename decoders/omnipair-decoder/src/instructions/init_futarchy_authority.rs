
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x856e9a1df0ce4764")]
pub struct InitFutarchyAuthority{
    pub args: InitFutarchyAuthorityArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct InitFutarchyAuthorityInstructionAccounts {
    pub deployer: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for InitFutarchyAuthority {
    type ArrangedAccounts = InitFutarchyAuthorityInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let deployer = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;

        Some(InitFutarchyAuthorityInstructionAccounts {
            deployer,
            futarchy_authority,
            system_program,
        })
    }
}