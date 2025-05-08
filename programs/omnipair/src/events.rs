use anchor_lang::prelude::*;

#[event]
pub struct SwapEvent {
    pub user: Pubkey,
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub amount0_out: u64,
    pub amount1_out: u64,
    pub timestamp: i64,
}

#[event]
pub struct AdjustCollateralEvent {
    pub user: Pubkey,
    pub amount0: i64,
    pub amount1: i64,
    pub timestamp: i64,
}

#[event]
pub struct AdjustDebtEvent {
    pub user: Pubkey,
    pub amount0: i64,
    pub amount1: i64,
    pub timestamp: i64,
}

#[event]
pub struct PairCreatedEvent {
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub pair: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct AdjustLiquidityEvent {
    pub user: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub timestamp: i64,
}

#[event]
pub struct BurnEvent {
    pub user: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub timestamp: i64,
}

#[event]
pub struct MintEvent {
    pub user: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub timestamp: i64,
}

#[event]
pub struct UpdatePairEvent {
    pub price0_ema: u64,
    pub price1_ema: u64,
    pub rate0: u64,
    pub rate1: u64,
    pub timestamp: i64,
}
