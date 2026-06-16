// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct LiquidityAdded {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub reserve_credit: u64,
    pub claim_amount: u64,
    pub buffer_amount: u64,
    pub protected_claim_token_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadata,
}
