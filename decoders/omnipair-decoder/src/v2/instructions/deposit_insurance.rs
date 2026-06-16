// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x22ddee67be8817c2")]
pub struct DepositInsurance {
    pub args: DepositInsuranceArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct DepositInsuranceInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub sponsor: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub insurance_vault: solana_pubkey::Pubkey,
    pub sponsor_asset_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for DepositInsurance {
    type ArrangedAccounts = DepositInsuranceInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let sponsor = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let insurance_vault = next_account(&mut iter)?;
        let sponsor_asset_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(DepositInsuranceInstructionAccounts {
            market,
            sponsor,
            asset_mint,
            insurance_vault,
            sponsor_asset_account,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
