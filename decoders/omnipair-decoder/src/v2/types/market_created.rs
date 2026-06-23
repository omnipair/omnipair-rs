// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct MarketCreated {
    pub market: solana_pubkey::Pubkey,
    pub base_mint: solana_pubkey::Pubkey,
    pub quote_mint: solana_pubkey::Pubkey,
    pub base_ylp_mint: solana_pubkey::Pubkey,
    pub quote_ylp_mint: solana_pubkey::Pubkey,
    pub base_collateral_vault: solana_pubkey::Pubkey,
    pub quote_collateral_vault: solana_pubkey::Pubkey,
    pub base_insurance_vault: solana_pubkey::Pubkey,
    pub quote_insurance_vault: solana_pubkey::Pubkey,
    pub base_hlp_mint: solana_pubkey::Pubkey,
    pub quote_hlp_mint: solana_pubkey::Pubkey,
    pub operator: solana_pubkey::Pubkey,
    pub manager: solana_pubkey::Pubkey,
    pub target_hlp_leverage_bps: u16,
    pub swap_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: MarketEventMetadata,
}
