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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MarketEventMetadataV2 {
    pub signer: Pubkey,
    pub market: Pubkey,
    pub slot: u64,
}

impl MarketEventMetadataV2 {
    pub fn new(signer: Pubkey, market: Pubkey) -> Self {
        Self {
            signer,
            market,
            slot: Clock::get().unwrap().slot,
        }
    }
}

#[event]
pub struct MarketCreatedV2 {
    pub market: Pubkey,
    pub asset0_mint: Pubkey,
    pub asset1_mint: Pubkey,
    pub claim0_mint: Pubkey,
    pub claim1_mint: Pubkey,
    pub claim0_stake_vault: Pubkey,
    pub claim1_stake_vault: Pubkey,
    pub hedge0_mint: Pubkey,
    pub hedge1_mint: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketUpdatedV2 {
    pub market: Pubkey,
    pub reduce_only: bool,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketHealthUpdatedV2 {
    pub market: Pubkey,
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub effective_debt0_nad: u128,
    pub effective_debt1_nad: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketReserveDepositedV2 {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub reserve_credit: u64,
    pub claim_amount: u64,
    pub buffer_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketClaimRedeemedV2 {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub claim_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketStakeUpdatedV2 {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub staked_claim_amount: u64,
    pub staked_buffer_shares: u64,
    pub active_stake_units: u64,
    pub accrued_fee_amount: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketFeesClaimedV2 {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct MarketSwapV2 {
    pub market: Pubkey,
    pub trader: Pubkey,
    pub asset_in_mint: Pubkey,
    pub asset_out_mint: Pubkey,
    pub reserve_credit: u64,
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub metadata: MarketEventMetadataV2,
}

#[event]
pub struct SwapEvent {
    pub reserve0: u64,
    pub reserve1: u64,
    pub is_token0_in: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub amount_in_after_fee: u64,
    /// Swap fee (input token units) to LPs
    pub lp_fee: u64,
    /// Swap fee (input token units) to protocol
    pub protocol_fee: u64,
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
    pub target_util_start_bps: u64,
    pub target_util_end_bps: u64,
    pub rate_half_life_ms: u64,
    pub min_rate_bps: u64,
    pub max_rate_bps: u64,
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
    pub cash_reserve0: u64,
    pub cash_reserve1: u64,
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
    /// Total interest (token0) applied to borrowers this update = lp_interest0 + protocol_interest0
    pub accrued_interest0: u128,
    /// Total interest (token1) applied to borrowers this update = lp_interest1 + protocol_interest1
    pub accrued_interest1: u128,
    /// Interest (token0) to LPs this update, added to reserves
    pub lp_interest0: u64,
    /// Interest (token1) to LPs this update, added to reserves
    pub lp_interest1: u64,
    /// Interest (token0) to protocol this update
    pub protocol_interest0: u64,
    /// Interest (token1) to protocol this update
    pub protocol_interest1: u64,
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
    pub debt0_shares: u128,
    pub debt1_shares: u128,
    pub collateral0_max_cf_bps: u16,
    pub collateral1_max_cf_bps: u16,
    pub collateral0_liquidation_cf_bps: u16,
    pub collateral1_liquidation_cf_bps: u16,
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

#[event]
pub struct ClaimProtocolFeesEvent {
    pub token0: Pubkey,
    pub token1: Pubkey,
    pub futarchy_treasury_amount0: u64,
    pub futarchy_treasury_amount1: u64,
    pub buybacks_vault_amount0: u64,
    pub buybacks_vault_amount1: u64,
    pub team_treasury_amount0: u64,
    pub team_treasury_amount1: u64,
    pub metadata: EventMetadata,
}
