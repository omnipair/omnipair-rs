

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0xdfb3e27d302e274a")]
pub struct Liquidate{
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct LiquidateInstructionAccounts {
    pub pair: solana_pubkey::Pubkey,
    pub user_position: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub collateral_vault: solana_pubkey::Pubkey,
    pub caller_token_account: solana_pubkey::Pubkey,
    pub collateral_token_mint: solana_pubkey::Pubkey,
    pub reserve_vault: solana_pubkey::Pubkey,
    pub position_owner: solana_pubkey::Pubkey,
    pub payer: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Liquidate {
    type ArrangedAccounts = LiquidateInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let pair = next_account(&mut iter)?;
        let user_position = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let collateral_vault = next_account(&mut iter)?;
        let caller_token_account = next_account(&mut iter)?;
        let collateral_token_mint = next_account(&mut iter)?;
        let reserve_vault = next_account(&mut iter)?;
        let position_owner = next_account(&mut iter)?;
        let payer = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(LiquidateInstructionAccounts {
            pair,
            user_position,
            rate_model,
            futarchy_authority,
            collateral_vault,
            caller_token_account,
            collateral_token_mint,
            reserve_vault,
            position_owner,
            payer,
            token_program,
            token_2022_program,
            system_program,
            event_authority,
            program,
        })
    }
}