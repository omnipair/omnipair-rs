// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x87966da5ae23a397")]
pub struct MarketDebtUpdated {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub debt_asset_mint: solana_pubkey::Pubkey,
    pub debt_delta: i64,
    pub fixed_base_debt: u128,
    pub fixed_quote_debt: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
    pub metadata: MarketEventMetadata,
}
