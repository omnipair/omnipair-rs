// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xdf6d06e588a02b2f")]
pub struct CloseHedge {
    pub args: CloseHedgeArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct CloseHedgeInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub base_ylp_mint: solana_pubkey::Pubkey,
    pub quote_ylp_mint: solana_pubkey::Pubkey,
    pub target_hlp_mint: solana_pubkey::Pubkey,
    pub base_reserve_vault: solana_pubkey::Pubkey,
    pub quote_reserve_vault: solana_pubkey::Pubkey,
    pub owner_target_account: solana_pubkey::Pubkey,
    pub owner_hlp_account: solana_pubkey::Pubkey,
    pub hlp_base_ylp_account: solana_pubkey::Pubkey,
    pub hlp_quote_ylp_account: solana_pubkey::Pubkey,
    pub target_yield_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for CloseHedge {
    type ArrangedAccounts = CloseHedgeInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let base_mint = next_account(&mut iter)?;
        let quote_mint = next_account(&mut iter)?;
        let base_ylp_mint = next_account(&mut iter)?;
        let quote_ylp_mint = next_account(&mut iter)?;
        let target_hlp_mint = next_account(&mut iter)?;
        let base_reserve_vault = next_account(&mut iter)?;
        let quote_reserve_vault = next_account(&mut iter)?;
        let owner_target_account = next_account(&mut iter)?;
        let owner_hlp_account = next_account(&mut iter)?;
        let hlp_base_ylp_account = next_account(&mut iter)?;
        let hlp_quote_ylp_account = next_account(&mut iter)?;
        let target_yield_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(CloseHedgeInstructionAccounts {
            market,
            owner,
            base_mint,
            quote_mint,
            base_ylp_mint,
            quote_ylp_mint,
            target_hlp_mint,
            base_reserve_vault,
            quote_reserve_vault,
            owner_target_account,
            owner_hlp_account,
            hlp_base_ylp_account,
            hlp_quote_ylp_account,
            target_yield_account,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}
