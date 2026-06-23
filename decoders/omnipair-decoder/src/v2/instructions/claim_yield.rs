// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x314a6f07ba163da5")]
pub struct ClaimYield {
    pub args: ClaimYieldArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClaimYieldInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub owner_lp_account: solana_pubkey::Pubkey,
    pub fee_vault: solana_pubkey::Pubkey,
    pub interest_vault: solana_pubkey::Pubkey,
    pub recipient_asset_account: solana_pubkey::Pubkey,
    pub yield_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for ClaimYield {
    type ArrangedAccounts = ClaimYieldInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let lp_mint = next_account(&mut iter)?;
        let owner_lp_account = next_account(&mut iter)?;
        let fee_vault = next_account(&mut iter)?;
        let interest_vault = next_account(&mut iter)?;
        let recipient_asset_account = next_account(&mut iter)?;
        let yield_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(ClaimYieldInstructionAccounts {
            market,
            owner,
            asset_mint,
            lp_mint,
            owner_lp_account,
            fee_vault,
            interest_vault,
            recipient_asset_account,
            yield_account,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
