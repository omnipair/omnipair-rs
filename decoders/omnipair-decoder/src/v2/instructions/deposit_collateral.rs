// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x9c838e7492f7a278")]
pub struct DepositCollateral {
    pub args: DepositCollateralArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct DepositCollateralInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub collateral_vault: solana_pubkey::Pubkey,
    pub owner_asset_account: solana_pubkey::Pubkey,
    pub margin_position: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for DepositCollateral {
    type ArrangedAccounts = DepositCollateralInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let collateral_vault = next_account(&mut iter)?;
        let owner_asset_account = next_account(&mut iter)?;
        let margin_position = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(DepositCollateralInstructionAccounts {
            market,
            owner,
            asset_mint,
            collateral_vault,
            owner_asset_account,
            margin_position,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}
