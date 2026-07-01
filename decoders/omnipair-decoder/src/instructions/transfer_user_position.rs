
use carbon_core::{CarbonDeserialize, account_utils::next_account, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x8a02d9c7d2229294")]
pub struct TransferUserPosition;

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct TransferUserPositionInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub from_position: solana_pubkey::Pubkey,
    pub to_position: solana_pubkey::Pubkey,
    pub current_owner: solana_pubkey::Pubkey,
    pub new_owner: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for TransferUserPosition {
    type ArrangedAccounts = TransferUserPositionInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let from_position = next_account(&mut iter)?;
        let to_position = next_account(&mut iter)?;
        let current_owner = next_account(&mut iter)?;
        let new_owner = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(TransferUserPositionInstructionAccounts {
            pair,
            from_position,
            to_position,
            current_owner,
            new_owner,
            system_program,
            event_authority,
            program,
        })
    }
}
