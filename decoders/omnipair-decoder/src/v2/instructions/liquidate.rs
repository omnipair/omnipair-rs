// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xdfb3e27d302e274a")]
pub struct Liquidate {
    pub args: LiquidateArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct LiquidateInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub liquidator: solana_pubkey::Pubkey,
    pub debt_asset_mint: solana_pubkey::Pubkey,
    pub collateral_asset_mint: solana_pubkey::Pubkey,
    pub reserve_vault: solana_pubkey::Pubkey,
    pub collateral_vault: solana_pubkey::Pubkey,
    pub insurance_vault: solana_pubkey::Pubkey,
    pub collateral_insurance_vault: solana_pubkey::Pubkey,
    pub liquidator_debt_account: solana_pubkey::Pubkey,
    pub liquidator_collateral_account: solana_pubkey::Pubkey,
    pub margin_position: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Liquidate {
    type ArrangedAccounts = LiquidateInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let liquidator = next_account(&mut iter)?;
        let debt_asset_mint = next_account(&mut iter)?;
        let collateral_asset_mint = next_account(&mut iter)?;
        let reserve_vault = next_account(&mut iter)?;
        let collateral_vault = next_account(&mut iter)?;
        let insurance_vault = next_account(&mut iter)?;
        let collateral_insurance_vault = next_account(&mut iter)?;
        let liquidator_debt_account = next_account(&mut iter)?;
        let liquidator_collateral_account = next_account(&mut iter)?;
        let margin_position = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(LiquidateInstructionAccounts {
            market,
            liquidator,
            debt_asset_mint,
            collateral_asset_mint,
            reserve_vault,
            collateral_vault,
            insurance_vault,
            collateral_insurance_vault,
            liquidator_debt_account,
            liquidator_collateral_account,
            margin_position,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
