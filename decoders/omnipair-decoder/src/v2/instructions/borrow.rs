// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xe4fd83cacf745912")]
pub struct Borrow {
    pub args: BorrowArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct BorrowInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub debt_asset_mint: solana_pubkey::Pubkey,
    pub collateral_asset_mint: solana_pubkey::Pubkey,
    pub reserve_vault: solana_pubkey::Pubkey,
    pub owner_debt_account: solana_pubkey::Pubkey,
    pub margin_position: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Borrow {
    type ArrangedAccounts = BorrowInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let debt_asset_mint = next_account(&mut iter)?;
        let collateral_asset_mint = next_account(&mut iter)?;
        let reserve_vault = next_account(&mut iter)?;
        let owner_debt_account = next_account(&mut iter)?;
        let margin_position = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(BorrowInstructionAccounts {
            market,
            futarchy_authority,
            owner,
            debt_asset_mint,
            collateral_asset_mint,
            reserve_vault,
            owner_debt_account,
            margin_position,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
