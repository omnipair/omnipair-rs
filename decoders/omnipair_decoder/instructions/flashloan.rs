
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x692101032a9ef643")]
pub struct Flashloan{
    pub args: FlashloanArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct FlashloanInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub token0_vault: solana_pubkey::Pubkey,
    pub token1_vault: solana_pubkey::Pubkey,
    pub token0_mint: solana_pubkey::Pubkey,
    pub token1_mint: solana_pubkey::Pubkey,
    pub receiver_token0_account: solana_pubkey::Pubkey,
    pub receiver_token1_account: solana_pubkey::Pubkey,
    pub receiver_program: solana_pubkey::Pubkey,
    pub user: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Flashloan {
    type ArrangedAccounts = FlashloanInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let token0_vault = next_account(&mut iter)?;
        let token1_vault = next_account(&mut iter)?;
        let token0_mint = next_account(&mut iter)?;
        let token1_mint = next_account(&mut iter)?;
        let receiver_token0_account = next_account(&mut iter)?;
        let receiver_token1_account = next_account(&mut iter)?;
        let receiver_program = next_account(&mut iter)?;
        let user = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(FlashloanInstructionAccounts {
            pair,
            rate_model,
            futarchy_authority,
            token0_vault,
            token1_vault,
            token0_mint,
            token1_mint,
            receiver_token0_account,
            receiver_token1_account,
            receiver_program,
            user,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}