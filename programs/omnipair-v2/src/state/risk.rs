use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DebtBook {
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub soft_debt0_shares: u128,
    pub soft_debt1_shares: u128,
    pub borrow_index0_nad: u128,
    pub borrow_index1_nad: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RiskBook {
    pub price0_ema_nad: u64,
    pub price1_ema_nad: u64,
    pub directional_price0_ema_nad: u64,
    pub directional_price1_ema_nad: u64,
    pub cached_spot_price0_nad: u64,
    pub cached_spot_price1_nad: u64,
    pub cached_k_nad: u128,
    pub cached_liquidity_nad: u128,
    pub cached_liquidity0_nad: u128,
    pub cached_liquidity1_nad: u128,
    pub k_ema: u128,
    pub liquidity_ema: u128,
    pub liquidity0_ema: u128,
    pub liquidity1_ema: u128,
    pub last_snapshot_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketHealth {
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub effective_debt0_nad: u128,
    pub effective_debt1_nad: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
}
