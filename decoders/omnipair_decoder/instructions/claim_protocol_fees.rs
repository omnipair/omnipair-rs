

use carbon_core::{CarbonDeserialize, borsh, account_utils::next_account};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
#[carbon(discriminator = "0x228edb706d368517")]
pub struct ClaimProtocolFees{
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClaimProtocolFeesInstructionAccounts {
    pub caller: solana_pubkey::Pubkey,
    pub pair: solana_pubkey::Pubkey,
    pub rate_model: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub reserve0_vault: solana_pubkey::Pubkey,
    pub reserve1_vault: solana_pubkey::Pubkey,
    pub authority_token0_account: solana_pubkey::Pubkey,
    pub authority_token1_account: solana_pubkey::Pubkey,
    pub token0_mint: solana_pubkey::Pubkey,
    pub token1_mint: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub associated_token_program: solana_pubkey::Pubkey,
    pub system_program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for ClaimProtocolFees {
    type ArrangedAccounts = ClaimProtocolFeesInstructionAccounts;

    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let caller = next_account(&mut iter)?;
        let pair = next_account(&mut iter)?;
        let rate_model = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let reserve0_vault = next_account(&mut iter)?;
        let reserve1_vault = next_account(&mut iter)?;
        let authority_token0_account = next_account(&mut iter)?;
        let authority_token1_account = next_account(&mut iter)?;
        let token0_mint = next_account(&mut iter)?;
        let token1_mint = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let associated_token_program = next_account(&mut iter)?;
        let system_program = next_account(&mut iter)?;

        Some(ClaimProtocolFeesInstructionAccounts {
            caller,
            pair,
            rate_model,
            futarchy_authority,
            reserve0_vault,
            reserve1_vault,
            authority_token0_account,
            authority_token1_account,
            token0_mint,
            token1_mint,
            token_program,
            token_2022_program,
            associated_token_program,
            system_program,
        })
    }
}