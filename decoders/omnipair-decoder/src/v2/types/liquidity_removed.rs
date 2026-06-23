// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct LiquidityRemoved {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub base_ylp_supply: u64,
    pub quote_ylp_supply: u64,
    pub metadata: MarketEventMetadata,
}
