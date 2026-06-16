// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xb069809c89e67eba")]
pub struct MarginPosition {
    pub owner: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub base_collateral: u64,
    pub quote_collateral: u64,
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub fixed_base_debt_shares: u128,
    pub fixed_quote_debt_shares: u128,
    pub bump: u8,
}
