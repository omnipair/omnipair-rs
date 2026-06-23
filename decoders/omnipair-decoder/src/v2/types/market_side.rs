// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct MarketSide {
    pub asset_mint: solana_pubkey::Pubkey,
    pub asset_decimals: u8,
    pub ylp_mint: solana_pubkey::Pubkey,
    pub hlp_mint: solana_pubkey::Pubkey,
    pub reserve_vault: solana_pubkey::Pubkey,
    pub collateral_vault: solana_pubkey::Pubkey,
    pub fee_vault: solana_pubkey::Pubkey,
    pub interest_vault: solana_pubkey::Pubkey,
    pub reserves: Reserves,
    pub shares: ReserveShares,
    pub fees: Fees,
    pub daily_limits: DailyLimits,
}
