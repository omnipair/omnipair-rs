// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xb578fee0e87130dd")]
pub struct ClaimMarketFees {
    pub args: ClaimMarketFeesArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClaimMarketFeesInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub fee_authority: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub fee_vault: solana_pubkey::Pubkey,
    pub recipient_fee_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for ClaimMarketFees {
    type ArrangedAccounts = ClaimMarketFeesInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let fee_authority = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let fee_vault = next_account(&mut iter)?;
        let recipient_fee_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(ClaimMarketFeesInstructionAccounts {
            market,
            fee_authority,
            asset_mint,
            fee_vault,
            recipient_fee_account,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
