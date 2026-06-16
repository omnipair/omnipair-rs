// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xceb0ca12c8d1b36c")]
pub struct Stake {
    pub args: StakeArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct StakeInstructionAccounts {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub claim_token_mint: solana_pubkey::Pubkey,
    pub stake_vault: solana_pubkey::Pubkey,
    pub owner_claim_account: solana_pubkey::Pubkey,
    pub stake_position: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Stake {
    type ArrangedAccounts = StakeInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let market = next_account(&mut iter)?;
        let owner = next_account(&mut iter)?;
        let asset_mint = next_account(&mut iter)?;
        let claim_token_mint = next_account(&mut iter)?;
        let stake_vault = next_account(&mut iter)?;
        let owner_claim_account = next_account(&mut iter)?;
        let stake_position = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(StakeInstructionAccounts {
            market,
            owner,
            asset_mint,
            claim_token_mint,
            stake_vault,
            owner_claim_account,
            stake_position,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
