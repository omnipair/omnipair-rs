// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x228edb706d368517")]
pub struct ClaimProtocolFees {}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClaimProtocolFeesInstructionAccounts {
    pub caller: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub base_fee_vault: solana_pubkey::Pubkey,
    pub quote_fee_vault: solana_pubkey::Pubkey,
    pub futarchy_treasury: solana_pubkey::Pubkey,
    pub buybacks_vault: solana_pubkey::Pubkey,
    pub team_treasury: solana_pubkey::Pubkey,
    pub futarchy_treasury_base_account: solana_pubkey::Pubkey,
    pub futarchy_treasury_quote_account: solana_pubkey::Pubkey,
    pub buybacks_vault_base_account: solana_pubkey::Pubkey,
    pub buybacks_vault_quote_account: solana_pubkey::Pubkey,
    pub team_treasury_base_account: solana_pubkey::Pubkey,
    pub team_treasury_quote_account: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for ClaimProtocolFees {
    type ArrangedAccounts = ClaimProtocolFeesInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let caller = next_account(&mut iter)?;
        let market = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let base_mint = next_account(&mut iter)?;
        let quote_mint = next_account(&mut iter)?;
        let base_fee_vault = next_account(&mut iter)?;
        let quote_fee_vault = next_account(&mut iter)?;
        let futarchy_treasury = next_account(&mut iter)?;
        let buybacks_vault = next_account(&mut iter)?;
        let team_treasury = next_account(&mut iter)?;
        let futarchy_treasury_base_account = next_account(&mut iter)?;
        let futarchy_treasury_quote_account = next_account(&mut iter)?;
        let buybacks_vault_base_account = next_account(&mut iter)?;
        let buybacks_vault_quote_account = next_account(&mut iter)?;
        let team_treasury_base_account = next_account(&mut iter)?;
        let team_treasury_quote_account = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(ClaimProtocolFeesInstructionAccounts {
            caller,
            market,
            futarchy_authority,
            base_mint,
            quote_mint,
            base_fee_vault,
            quote_fee_vault,
            futarchy_treasury,
            buybacks_vault,
            team_treasury,
            futarchy_treasury_base_account,
            futarchy_treasury_quote_account,
            buybacks_vault_base_account,
            buybacks_vault_quote_account,
            team_treasury_base_account,
            team_treasury_quote_account,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
