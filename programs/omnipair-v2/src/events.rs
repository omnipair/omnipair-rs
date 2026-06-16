use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MarketEventMetadata {
    pub signer: Pubkey,
    pub market: Pubkey,
    pub slot: u64,
}

impl MarketEventMetadata {
    pub fn new(signer: Pubkey, market: Pubkey) -> Self {
        Self {
            signer,
            market,
            slot: Clock::get().unwrap().slot,
        }
    }
}

#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub asset0_mint: Pubkey,
    pub asset1_mint: Pubkey,
    pub claim0_mint: Pubkey,
    pub claim1_mint: Pubkey,
    pub claim0_stake_vault: Pubkey,
    pub claim1_stake_vault: Pubkey,
    pub collateral0_vault: Pubkey,
    pub collateral1_vault: Pubkey,
    pub insurance0_vault: Pubkey,
    pub insurance1_vault: Pubkey,
    pub hedge0_mint: Pubkey,
    pub hedge1_mint: Pubkey,
    pub hedge0_vault: Pubkey,
    pub hedge1_vault: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketUpdated {
    pub market: Pubkey,
    pub reduce_only: bool,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHealthUpdated {
    pub market: Pubkey,
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub effective_debt0_nad: u128,
    pub effective_debt1_nad: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketReserveDeposited {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub reserve_credit: u64,
    pub claim_amount: u64,
    pub buffer_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketClaimRedeemed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub claim_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketStakeUpdated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub staked_claim_amount: u64,
    pub staked_buffer_shares: u64,
    pub active_stake_units: u64,
    pub accrued_fee_amount: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketFeesClaimed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketFeeLiabilityClaimed {
    pub market: Pubkey,
    pub authority: Pubkey,
    pub asset_mint: Pubkey,
    pub claim_kind: u8,
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketSwapEvent {
    pub market: Pubkey,
    pub trader: Pubkey,
    pub asset_in_mint: Pubkey,
    pub asset_out_mint: Pubkey,
    pub reserve_credit: u64,
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketCollateralDeposited {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub collateral_credit: u64,
    pub collateral0: u64,
    pub collateral1: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketCollateralWithdrawn {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub collateral_debit: u64,
    pub asset_credit: u64,
    pub collateral0: u64,
    pub collateral1: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketDebtUpdated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub debt_delta: i64,
    pub fixed_debt0: u128,
    pub fixed_debt1: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketInsuranceFunded {
    pub market: Pubkey,
    pub sponsor: Pubkey,
    pub asset_mint: Pubkey,
    pub insurance_credit: u64,
    pub available0: u64,
    pub available1: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketLiquidated {
    pub market: Pubkey,
    pub borrower: Pubkey,
    pub liquidator: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub collateral_asset_mint: Pubkey,
    pub repaid_amount: u64,
    pub collateral_seized: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHedgeOpened {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub claim_amount: u64,
    pub hedge_amount: u64,
    pub hedged_claim_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHedgeClosed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub hedge_amount: u64,
    pub claim_amount: u64,
    pub hedged_claim_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHedgeFeesClaimed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub metadata: MarketEventMetadata,
}
