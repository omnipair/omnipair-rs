use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CommonFields {
    pub signer: Pubkey,
    pub pair: Pubkey,
    pub timestamp: i64,
}

impl CommonFields {
    pub fn new(signer: Pubkey, pair: Pubkey) -> Self {
        Self {
            signer,
            pair,
            timestamp: Clock::get().unwrap().unix_timestamp,
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
    pub common: CommonFields,
}

#[event]
pub struct AdjustCollateralEvent {
    pub amount0: i64,
    pub amount1: i64,
    pub common: CommonFields,
}

#[event]
pub struct AdjustDebtEvent {
    pub amount0: i64,
    pub amount1: i64,
    pub common: CommonFields,
}

#[event]
pub struct PairCreatedEvent {
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub common: CommonFields,
}

#[event]
pub struct AdjustLiquidityEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub common: CommonFields,
}

#[event]
pub struct BurnEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub common: CommonFields,
}

#[event]
pub struct MintEvent {
    pub amount0: u64,
    pub amount1: u64,
    pub liquidity: u64,
    pub common: CommonFields,
}

#[event]
pub struct UpdatePairEvent {
    pub price0_ema: u64,
    pub price1_ema: u64,
    pub rate0: u64,
    pub rate1: u64,
    pub common: CommonFields,
}

#[event]
pub struct UserPositionCreatedEvent {
    pub position: Pubkey,
    pub common: CommonFields,
}

#[event]
pub struct UserPositionUpdatedEvent {
    pub position: Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub debt0_shares: u64,
    pub debt1_shares: u64,
    pub common: CommonFields,
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
    pub common: CommonFields,
}
