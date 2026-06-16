// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x3fd1114ad9ced6c1")]
pub struct MarketStakeUpdated {
    pub market: solana_pubkey::Pubkey,
    pub owner: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub staked_claim_token_amount: u64,
    pub staked_buffer_share_amount: u64,
    pub active_stake_units: u64,
    pub accrued_fee_amount: u64,
    pub metadata: MarketEventMetadata,
}
