// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct FutarchyAuthority {
    pub version: u8,
    pub authority: solana_pubkey::Pubkey,
    pub recipients: RevenueRecipients,
    pub revenue_share: RevenueShare,
    pub revenue_distribution: RevenueDistribution,
    pub global_reduce_only: bool,
    pub bump: u8,
}
