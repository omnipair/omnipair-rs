// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xbbe9d0f9a068d175")]
pub struct SetReduceOnly {
    pub args: SetMarketReduceOnlyArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SetReduceOnlyInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub authority_signer: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for SetReduceOnly {
    type ArrangedAccounts = SetReduceOnlyInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let authority_signer = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(SetReduceOnlyInstructionAccounts {
            market,
            authority_signer,
            event_authority,
            program,
        })
    }
}
