use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MarketEventMetadata {
    pub signer: Pubkey,
    pub market: Pubkey,
    pub slot: u64,
}

impl MarketEventMetadata {
    pub fn new(signer: Pubkey, market: Pubkey) -> Result<Self> {
        Ok(Self {
            signer,
            market,
            slot: Clock::get()?.slot,
        })
    }
}

#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_claim_token_mint: Pubkey,
    pub quote_claim_token_mint: Pubkey,
    pub base_stake_vault: Pubkey,
    pub quote_stake_vault: Pubkey,
    pub base_collateral_vault: Pubkey,
    pub quote_collateral_vault: Pubkey,
    pub base_insurance_vault: Pubkey,
    pub quote_insurance_vault: Pubkey,
    pub base_hedge_token_mint: Pubkey,
    pub quote_hedge_token_mint: Pubkey,
    pub base_hedge_vault: Pubkey,
    pub quote_hedge_vault: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub buffer_ratio_bps: u16,
    pub swap_fee_bps: u16,
    pub protocol_fee_bps: u16,
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
    pub protocol_fee_bps: u16,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHealthUpdated {
    pub market: Pubkey,
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub effective_base_debt_nad: u128,
    pub effective_quote_debt_nad: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct LiquidityAdded {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub reserve_credit: u64,
    pub claim_amount: u64,
    pub buffer_amount: u64,
    pub protected_claim_token_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct LiquidityRemoved {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub claim_amount: u64,
    pub protected_claim_token_supply: u64,
    pub required_buffer: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketStakeUpdated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub staked_claim_token_amount: u64,
    pub staked_buffer_share_amount: u64,
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
pub struct SwapExecuted {
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
    pub base_collateral: u64,
    pub quote_collateral: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketCollateralWithdrawn {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub collateral_debit: u64,
    pub asset_credit: u64,
    pub base_collateral: u64,
    pub quote_collateral: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketDebtUpdated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub debt_delta: i64,
    pub fixed_base_debt: u128,
    pub fixed_quote_debt: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketInsuranceFunded {
    pub market: Pubkey,
    pub sponsor: Pubkey,
    pub asset_mint: Pubkey,
    pub insurance_credit: u64,
    pub base_available: u64,
    pub quote_available: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct PositionLiquidated {
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
    pub hedged_claim_token_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketHedgeClosed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub hedge_amount: u64,
    pub claim_amount: u64,
    pub hedged_claim_token_supply: u64,
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
