use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct MarginPosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub available_buffer_shares: u64,
    pub staked_claim_amount: u64,
    pub staked_buffer_shares: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct HedgePosition {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub hedged_claim_amount: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
    pub bump: u8,
}
