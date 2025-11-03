
use super::super::types::*;

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xafaf6d1f0d989bed")]
pub struct Initialize{
    pub args: InitializeAndBootstrapArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct InitializeInstructionAccounts {
    pub deployer: solana_pubkey::Pubkey,
    pub token0_mint: solana_pubkey::Pubkey,
    pub token1_mint: solana_pubkey::Pubkey,
    pub pair: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub lp_mint: solana_pubkey::Pubkey,
    pub deployer_lp_token_account: solana_pubkey::Pubkey,
    pub token0_vault: solana_pubkey::Pubkey,
    pub token1_vault: solana_pubkey::Pubkey,
    pub deployer_token0_account: solana_pubkey::Pubkey,
    pub deployer_token1_account: solana_pubkey::Pubkey,
    pub authority_wsol_account: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub associated_token_program: solana_pubkey::Pubkey,
    pub rent: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Initialize {
    type ArrangedAccounts = InitializeInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let deployer = next_account(&mut iter)?;
        let token0_mint = next_account(&mut iter)?;
        let token1_mint = next_account(&mut iter)?;
        let pair = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let lp_mint = next_account(&mut iter)?;
        let deployer_lp_token_account = next_account(&mut iter)?;
        let token0_vault = next_account(&mut iter)?;
        let token1_vault = next_account(&mut iter)?;
        let deployer_token0_account = next_account(&mut iter)?;
        let deployer_token1_account = next_account(&mut iter)?;
        let authority_wsol_account = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let associated_token_program = next_account(&mut iter)?;
        let rent = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(InitializeInstructionAccounts {
            deployer,
            token0_mint,
            token1_mint,
            pair,
            futarchy_authority,
            rate_model,
            lp_mint,
            deployer_lp_token_account,
            token0_vault,
            token1_vault,
            deployer_token0_account,
            deployer_token1_account,
            authority_wsol_account,
            system_program,
            token_program,
            token_2022_program,
            associated_token_program,
            rent,
            event_authority,
            program,
        })
    }
}