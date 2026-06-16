// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x4cd1626b4025c5a8")]
pub struct OpenHedge {
    pub args: OpenHedgeArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct OpenHedgeInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub claim_token_mint: solana_pubkey::Pubkey,
    pub hedge_token_mint: solana_pubkey::Pubkey,
    pub hedge_vault: solana_pubkey::Pubkey,
    pub owner_claim_account: solana_pubkey::Pubkey,
    pub owner_hedge_account: solana_pubkey::Pubkey,
    pub hedge_position: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for OpenHedge {
    type ArrangedAccounts = OpenHedgeInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let claim_token_mint = next_account(&mut iter)?;
        let hedge_token_mint = next_account(&mut iter)?;
        let hedge_vault = next_account(&mut iter)?;
        let owner_claim_account = next_account(&mut iter)?;
        let owner_hedge_account = next_account(&mut iter)?;
        let hedge_position = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(OpenHedgeInstructionAccounts {
            market,
            owner,
            asset_mint,
            claim_token_mint,
            hedge_token_mint,
            hedge_vault,
            owner_claim_account,
            owner_hedge_account,
            hedge_position,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}
