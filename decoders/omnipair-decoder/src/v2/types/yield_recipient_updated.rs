// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct YieldRecipientUpdated {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub token_kind: u8,
    pub recipient: solana_pubkey::Pubkey,
    pub metadata: MarketEventMetadata,
}
