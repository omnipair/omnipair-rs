// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xdbbed53700e3c69a")]
pub struct Market {
    pub version: u8,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub operator: solana_pubkey::Pubkey,
    pub manager: solana_pubkey::Pubkey,
    pub base_side: MarketSide,
    pub quote_side: MarketSide,
    pub config: MarketConfig,
    pub debt_book: DebtBook,
    pub risk_book: RiskBook,
    pub health: MarketHealth,
    pub recognition_ledger: RecognitionLedger,
    pub insurance_reserve: InsuranceReserve,
    pub params_hash: [u8; 32],
    pub last_update_slot: u64,
    pub reduce_only: bool,
    pub bump: u8,
}
