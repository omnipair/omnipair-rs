// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x04ca71c2d07ad449")]
pub struct UpdateProtocolAuctionConfig {
    pub args: UpdateProtocolAuctionConfigArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct UpdateProtocolAuctionConfigInstructionAccounts {
    pub authority_signer: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for UpdateProtocolAuctionConfig {
    type ArrangedAccounts = UpdateProtocolAuctionConfigInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let authority_signer = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;

        Some(UpdateProtocolAuctionConfigInstructionAccounts {
            authority_signer,
            futarchy_authority,
            system_program,
        })
    }
}
