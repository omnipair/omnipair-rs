// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::super::types::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0x16e4cdfc39119cfc")]
pub struct ProtocolFeesClaimed {
    pub market: solana_pubkey::Pubkey,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub futarchy_treasury_base_amount: u64,
    pub futarchy_treasury_quote_amount: u64,
    pub buybacks_vault_base_amount: u64,
    pub buybacks_vault_quote_amount: u64,
    pub team_treasury_base_amount: u64,
    pub team_treasury_quote_amount: u64,
    pub metadata: MarketEventMetadata,
}
