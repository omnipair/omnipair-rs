
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x74b3892f76a741d9")]
pub struct UpdateRevenueRecipients{
    pub args: UpdateRevenueRecipientsArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct UpdateRevenueRecipientsInstructionAccounts {
    pub authority_signer: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for UpdateRevenueRecipients {
    type ArrangedAccounts = UpdateRevenueRecipientsInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let authority_signer = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;

        Some(UpdateRevenueRecipientsInstructionAccounts {
            authority_signer,
            futarchy_authority,
            system_program,
        })
    }
}