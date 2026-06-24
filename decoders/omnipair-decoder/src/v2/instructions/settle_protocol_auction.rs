// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xcecc208708164850")]
pub struct SettleProtocolAuction {
    pub args: SettleProtocolAuctionArgs,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SettleProtocolAuctionInstructionAccounts {
    pub bidder: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub futarchy_authority: solana_pubkey::Pubkey,
    pub sold_mint: solana_pubkey::Pubkey,
    pub accepted_mint: solana_pubkey::Pubkey,
    pub sold_fee_vault: solana_pubkey::Pubkey,
    pub bidder_payment_account: solana_pubkey::Pubkey,
    pub bidder_receive_account: solana_pubkey::Pubkey,
    pub treasury_payment_account: solana_pubkey::Pubkey,
    pub staking_vault_payment_account: solana_pubkey::Pubkey,
    pub reference_market: solana_pubkey::Pubkey,
    pub token_program: solana_pubkey::Pubkey,
    pub token_2022_program: solana_pubkey::Pubkey,
    pub event_authority: solana_pubkey::Pubkey,
    pub program: solana_pubkey::Pubkey,
}

impl carbon_core::deserialize::ArrangeAccounts for SettleProtocolAuction {
    type ArrangedAccounts = SettleProtocolAuctionInstructionAccounts;

    fn arrange_accounts(
        accounts: &[solana_instruction::AccountMeta],
    ) -> Option<Self::ArrangedAccounts> {
        let mut iter = accounts.iter();
        let bidder = next_account(&mut iter)?;
        let market = next_account(&mut iter)?;
        let futarchy_authority = next_account(&mut iter)?;
        let sold_mint = next_account(&mut iter)?;
        let accepted_mint = next_account(&mut iter)?;
        let sold_fee_vault = next_account(&mut iter)?;
        let bidder_payment_account = next_account(&mut iter)?;
        let bidder_receive_account = next_account(&mut iter)?;
        let treasury_payment_account = next_account(&mut iter)?;
        let staking_vault_payment_account = next_account(&mut iter)?;
        let reference_market = next_account(&mut iter)?;
        let token_program = next_account(&mut iter)?;
        let token_2022_program = next_account(&mut iter)?;
        let event_authority = next_account(&mut iter)?;
        let program = next_account(&mut iter)?;

        Some(SettleProtocolAuctionInstructionAccounts {
            bidder,
            market,
            futarchy_authority,
            sold_mint,
            accepted_mint,
            sold_fee_vault,
            bidder_payment_account,
            bidder_receive_account,
            treasury_payment_account,
            staking_vault_payment_account,
            reference_market,
            token_program,
            token_2022_program,
            event_authority,
            program,
        })
    }
}
