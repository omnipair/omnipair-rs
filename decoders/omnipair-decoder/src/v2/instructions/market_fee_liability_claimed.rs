// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x08dede432c6fda08")]
pub struct MarketFeeLiabilityClaimed {
    pub market: solana_pubkey::Pubkey,
    pub authority: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub claim_kind: u8,
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub metadata: MarketEventMetadata,
}
