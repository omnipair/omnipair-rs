// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct StakePosition {
    pub owner: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub available_buffer_share_amount: u64,
    pub staked_claim_token_amount: u64,
    pub staked_buffer_share_amount: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
    pub bump: u8,
}
