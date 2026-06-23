// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xafaf6d1f0d989bed")]
pub struct Initialize {
    pub args: InitializeMarketArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct InitializeInstructionAccounts {
    pub payer: solana_pubkey::Pubkey,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub base_ylp_mint: solana_pubkey::Pubkey,
    pub quote_ylp_mint: solana_pubkey::Pubkey,
    pub base_hlp_mint: solana_pubkey::Pubkey,
    pub quote_hlp_mint: solana_pubkey::Pubkey,
    pub base_reserve_vault: solana_pubkey::Pubkey,
    pub quote_reserve_vault: solana_pubkey::Pubkey,
    pub base_collateral_vault: solana_pubkey::Pubkey,
    pub quote_collateral_vault: solana_pubkey::Pubkey,
    pub base_insurance_vault: solana_pubkey::Pubkey,
    pub quote_insurance_vault: solana_pubkey::Pubkey,
    pub base_fee_vault: solana_pubkey::Pubkey,
    pub quote_fee_vault: solana_pubkey::Pubkey,
    pub base_interest_vault: solana_pubkey::Pubkey,
    pub quote_interest_vault: solana_pubkey::Pubkey,
    pub team_treasury: solana_pubkey::Pubkey,
    pub team_treasury_wsol_account: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for Initialize {
    type ArrangedAccounts = InitializeInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let payer = next_account(&mut iter)?;
        let base_mint = next_account(&mut iter)?;
        let quote_mint = next_account(&mut iter)?;
        let market = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let base_ylp_mint = next_account(&mut iter)?;
        let quote_ylp_mint = next_account(&mut iter)?;
        let base_hlp_mint = next_account(&mut iter)?;
        let quote_hlp_mint = next_account(&mut iter)?;
        let base_reserve_vault = next_account(&mut iter)?;
        let quote_reserve_vault = next_account(&mut iter)?;
        let base_collateral_vault = next_account(&mut iter)?;
        let quote_collateral_vault = next_account(&mut iter)?;
        let base_insurance_vault = next_account(&mut iter)?;
        let quote_insurance_vault = next_account(&mut iter)?;
        let base_fee_vault = next_account(&mut iter)?;
        let quote_fee_vault = next_account(&mut iter)?;
        let base_interest_vault = next_account(&mut iter)?;
        let quote_interest_vault = next_account(&mut iter)?;
        let team_treasury = next_account(&mut iter)?;
        let team_treasury_wsol_account = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(InitializeInstructionAccounts {
            payer,
            base_mint,
            quote_mint,
            market,
            futarchy_authority,
            base_ylp_mint,
            quote_ylp_mint,
            base_hlp_mint,
            quote_hlp_mint,
            base_reserve_vault,
            quote_reserve_vault,
            base_collateral_vault,
            quote_collateral_vault,
            base_insurance_vault,
            quote_insurance_vault,
            base_fee_vault,
            quote_fee_vault,
            base_interest_vault,
            quote_interest_vault,
            team_treasury,
            team_treasury_wsol_account,
            system_program,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
