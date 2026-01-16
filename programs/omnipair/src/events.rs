use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct EventMetadata {
    pub signer: Pubkey,
    pub pair: Pubkey,
    pub slot: u64,
}

impl EventMetadata {
    pub fn new(signer: Pubkey, pair: Pubkey) -> Self {
        Self {
            signer,
            pair,
            slot: Clock::get().unwrap().slot,
        }
    }
}

#[event]
pub struct SwapEvent {
    pub reserve0: u64,
    pub reserve1: u64,
    pub is_token0_in: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub amount_in_after_fee: u64,
    pub metadata: EventMetadata,
}

#[event]
pub struct AdjustCollateralEvent {
    pub amount0: i64,
    pub amount1: i64,
    pub metadata: EventMetadata,
}

#[event]
pub struct AdjustDebtEvent {
    pub amount0: i64,
    pub amount1: i64,
    pub metadata: EventMetadata,
}

#[event]
pub struct PairCreatedEvent {
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub lp_mint: Pubkey,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub rate_model: Pubkey,
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: EventMetadata,
}

#[event]
pub struct AdjustLiquidityEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub metadata: EventMetadata,
}

#[event]
pub struct BurnEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub metadata: EventMetadata,
}

#[event]
pub struct MintEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub metadata: EventMetadata,
}

#[event]
pub struct UserLiquidityPositionUpdatedEvent {
    pub token0_amount: u64,
    pub token1_amount: u64,
    pub lp_amount: u64,
    pub token0_mint: Pubkey,
    pub token1_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub metadata: EventMetadata,
}

#[event]
pub struct UpdatePairEvent {
    pub price0_ema: u64,
    pub price1_ema: u64,
    pub rate0: u64,
    pub rate1: u64,
    pub accrued_interest0: u128,
    pub accrued_interest1: u128,
    pub cash_reserve0: u64,
    pub cash_reserve1: u64,
    pub reserve0_after_interest: u64,
    pub reserve1_after_interest: u64,
    pub metadata: EventMetadata,
}

#[event]
pub struct UserPositionCreatedEvent {
    pub position: Pubkey,
    pub metadata: EventMetadata,
}

#[event]
pub struct UserPositionUpdatedEvent {
    pub position: Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub debt0_shares: u64,
    pub debt1_shares: u64,
    pub collateral0_applied_min_cf_bps: u16,
    pub collateral1_applied_min_cf_bps: u16,
    pub metadata: EventMetadata,
}

#[event]
pub struct UserPositionLiquidatedEvent {
    pub position: Pubkey,
    pub liquidator: Pubkey,
    pub collateral0_liquidated: u64,
    pub collateral1_liquidated: u64,
    pub debt0_liquidated: u64,
    pub debt1_liquidated: u64,
    pub collateral_price: u64,
    pub shortfall: u128,
    pub liquidation_bonus_applied: u64,
    pub k0: u128,
    pub k1: u128,
    pub metadata: EventMetadata,
}

#[event]
pub struct FlashloanEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub fee0: u64,
    pub fee1: u64,
    pub receiver: Pubkey,
    pub metadata: EventMetadata,
}