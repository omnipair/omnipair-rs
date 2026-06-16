// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xadaaf601e808b610")]
pub struct MarketInsuranceFunded {
    pub market: solana_pubkey::Pubkey,
    pub sponsor: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub insurance_credit: u64,
    pub base_available: u64,
    pub quote_available: u64,
    pub metadata: MarketEventMetadata,
}
