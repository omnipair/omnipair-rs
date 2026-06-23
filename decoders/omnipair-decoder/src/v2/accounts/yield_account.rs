// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
#[carbon(discriminator = "0xe9f17706020e6a9c")]
pub struct YieldAccount {
    pub owner: solana_pubkey::Pubkey,
    pub market: solana_pubkey::Pubkey,
    pub asset_mint: solana_pubkey::Pubkey,
    pub token_kind: u8,
    pub recipient: solana_pubkey::Pubkey,
    pub swap_fee_checkpoint_nad: u128,
    pub interest_checkpoint_nad: u128,
    pub accrued_swap_fee_amount: u64,
    pub accrued_interest_amount: u64,
    pub bump: u8,
}
