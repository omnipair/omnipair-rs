
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xb08b83c528e17dc8")]
pub struct UpdateProtocolRevenue{
    pub args: UpdateProtocolRevenueArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct UpdateProtocolRevenueInstructionAccounts {
    pub authority_signer: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for UpdateProtocolRevenue {
    type ArrangedAccounts = UpdateProtocolRevenueInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let authority_signer = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;

        Some(UpdateProtocolRevenueInstructionAccounts {
            authority_signer,
            futarchy_authority,
            system_program,
        })
    }
}