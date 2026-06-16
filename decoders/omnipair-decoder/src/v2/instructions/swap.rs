// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xf8c69e91e17587c8")]
pub struct Swap {
    pub args: SwapArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SwapInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub trader: solana_pubkey::Pubkey,
    pub asset_in_mint: solana_pubkey::Pubkey,
    pub asset_out_mint: solana_pubkey::Pubkey,
    pub reserve_in_vault: solana_pubkey::Pubkey,
    pub reserve_out_vault: solana_pubkey::Pubkey,
    pub fee_in_vault: solana_pubkey::Pubkey,
    pub trader_asset_in_account: solana_pubkey::Pubkey,
    pub trader_asset_out_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Swap {
    type ArrangedAccounts = SwapInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let trader = next_account(&mut iter)?;
        let asset_in_mint = next_account(&mut iter)?;
        let asset_out_mint = next_account(&mut iter)?;
        let reserve_in_vault = next_account(&mut iter)?;
        let reserve_out_vault = next_account(&mut iter)?;
        let fee_in_vault = next_account(&mut iter)?;
        let trader_asset_in_account = next_account(&mut iter)?;
        let trader_asset_out_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(SwapInstructionAccounts {
            market,
            trader,
            asset_in_mint,
            asset_out_mint,
            reserve_in_vault,
            reserve_out_vault,
            fee_in_vault,
            trader_asset_in_account,
            trader_asset_out_account,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
