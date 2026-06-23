// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct HlpVault {
    pub target_side: u8,
    pub base_ylp_vault: solana_pubkey::Pubkey,
    pub quote_ylp_vault: solana_pubkey::Pubkey,
    pub ylp_base_shares: u64,
    pub ylp_quote_shares: u64,
    pub debt_shares: u128,
    pub hlp_supply: u64,
    pub pending_rebalance: i128,
    pub base_swap_fee_growth_index_nad: u128,
    pub base_interest_growth_index_nad: u128,
    pub quote_swap_fee_growth_index_nad: u128,
    pub quote_interest_growth_index_nad: u128,
    pub base_swap_fee_checkpoint_nad: u128,
    pub base_interest_checkpoint_nad: u128,
    pub quote_swap_fee_checkpoint_nad: u128,
    pub quote_interest_checkpoint_nad: u128,
    pub unallocated_base_swap_fee_amount: u64,
    pub unallocated_base_interest_amount: u64,
    pub unallocated_quote_swap_fee_amount: u64,
    pub unallocated_quote_interest_amount: u64,
    pub last_nav_nad: u128,
    pub cached_settlement_price_nad: u128,
    pub last_rebalance_slot: u64,
}
