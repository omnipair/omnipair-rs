// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xb2d3500a8a34bc16")]
pub struct SetYieldRecipient {
    pub args: SetYieldRecipientArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SetYieldRecipientInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub yield_account: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for SetYieldRecipient {
    type ArrangedAccounts = SetYieldRecipientInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let yield_account = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(SetYieldRecipientInstructionAccounts {
            market,
            owner,
            asset_mint,
            yield_account,
            event_authority,
            program,
        })
    }
}
