// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct ProtocolAuctionSettled {
    pub market: solana_pubkey::Pubkey,
    pub reference_market: solana_pubkey::Pubkey,
    pub lane: u8,
    pub side: u8,
    pub bidder: solana_pubkey::Pubkey,
    pub sold_mint: solana_pubkey::Pubkey,
    pub accepted_mint: solana_pubkey::Pubkey,
    pub sold_amount: u64,
    pub payment_amount: u64,
    pub treasury_amount: u64,
    pub staking_vault_amount: u64,
    pub reference_price_nad: u64,
    pub auction_price_nad: u64,
    pub remaining_fee_liability: u64,
    pub remaining_buyback_liability: u64,
    pub metadata: MarketEventMetadata,
}
