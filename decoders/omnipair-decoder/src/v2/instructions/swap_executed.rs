// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x96a61ae11c59264f")]
pub struct SwapExecuted {
    pub market: solana_pubkey::Pubkey,
    pub trader: solana_pubkey::Pubkey,
    pub asset_in_mint: solana_pubkey::Pubkey,
    pub asset_out_mint: solana_pubkey::Pubkey,
    pub reserve_credit: u64,
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub metadata: MarketEventMetadata,
}
