
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x1ee7a94913a12cfc")]
pub struct ViewPairData{
    pub getter: PairViewKind,
    pub args: EmitValueArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct ViewPairDataInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for ViewPairData {
    type ArrangedAccounts = ViewPairDataInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;

        Some(ViewPairDataInstructionAccounts {
            pair,
            rate_model,
            futarchy_authority,
        })
    }
}