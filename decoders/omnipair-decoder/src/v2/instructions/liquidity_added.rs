// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x9a1add6cee40d9a1")]
pub struct LiquidityAdded {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub base_ylp_supply: u64,
    pub quote_ylp_supply: u64,
    pub metadata: MarketEventMetadata,
}
