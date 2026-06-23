// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct HlpClosed {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub hlp_amount: u64,
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub target_amount_out: u64,
    pub debt_repaid: u64,
    pub hlp_supply: u64,
    pub metadata: MarketEventMetadata,
}
