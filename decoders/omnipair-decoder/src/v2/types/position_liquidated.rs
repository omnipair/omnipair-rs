// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct PositionLiquidated {
    pub market: solana_pubkey::Pubkey,
    pub borrower: solana_pubkey::Pubkey,
    pub liquidator: solana_pubkey::Pubkey,
    pub debt_asset_mint: solana_pubkey::Pubkey,
    pub collateral_asset_mint: solana_pubkey::Pubkey,
    pub repaid_amount: u64,
    pub collateral_seized: u64,
    pub collateral_to_liquidator: u64,
    pub insurance_funded: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
    pub metadata: MarketEventMetadata,
}
