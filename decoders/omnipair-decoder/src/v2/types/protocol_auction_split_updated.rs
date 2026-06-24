// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct ProtocolAuctionSplitUpdated {
    pub authority: solana_pubkey::Pubkey,
    pub fee_auction_bps: u16,
    pub buyback_auction_bps: u16,
    pub signer: solana_pubkey::Pubkey,
}
