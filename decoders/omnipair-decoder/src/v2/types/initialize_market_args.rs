// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct InitializeMarketArgs {
    pub operator: solana_pubkey::Pubkey,
    pub manager: solana_pubkey::Pubkey,
    pub config: MarketConfig,
    pub params_hash: [u8; 32],
}
