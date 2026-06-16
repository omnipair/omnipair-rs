// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct MarketSide {
    pub asset_mint: solana_pubkey::Pubkey,
    pub asset_decimals: u8,
    pub claim_token_mint: solana_pubkey::Pubkey,
    pub hedge_token_mint: solana_pubkey::Pubkey,
    pub hedge_vault: solana_pubkey::Pubkey,
    pub reserve_vault: solana_pubkey::Pubkey,
    pub collateral_vault: solana_pubkey::Pubkey,
    pub fee_vault: solana_pubkey::Pubkey,
    pub stake_vault: solana_pubkey::Pubkey,
    pub reserve_ledger: ReserveLedger,
    pub claim_token_ledger: ClaimTokenLedger,
    pub buffer_ledger: BufferLedger,
    pub fee_ledger: FeeLedger,
    pub daily_limit_book: DailyLimitBook,
}
