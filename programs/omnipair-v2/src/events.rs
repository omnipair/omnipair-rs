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
    pub ylp_mint: Pubkey,
    pub base_collateral_vault: Pubkey,
    pub quote_collateral_vault: Pubkey,
    pub base_insurance_vault: Pubkey,
    pub quote_insurance_vault: Pubkey,
    pub base_hlp_mint: Pubkey,
    pub quote_hlp_mint: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub target_hlp_leverage_bps: u16,
    pub swap_fee_bps: u16,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub params_hash: [u8; 32],
    pub version: u8,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketUpdated {
    pub market: Pubkey,
    pub reduce_only: bool,
    pub target_hlp_leverage_bps: u16,
    pub swap_fee_bps: u16,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketConfigUpdateScheduled {
    pub market: Pubkey,
    pub execute_after_slot: u64,
    pub target_hlp_leverage_bps: u16,
    pub swap_fee_bps: u16,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketAuthorityUpdated {
    pub market: Pubkey,
    pub manager: Pubkey,
    pub operator: Pubkey,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct MarketAuthorityUpdateScheduled {
    pub market: Pubkey,
    pub role: u8,
    pub pending_authority: Pubkey,
    pub execute_after_slot: u64,
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
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
    pub ylp_amount: u64,
    pub ylp_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct LiquidityRemoved {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub ylp_amount: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub ylp_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct YieldRecipientUpdated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub token_kind: u8,
    pub recipient: Pubkey,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct YieldClaimed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub token_kind: u8,
    pub recipient: Pubkey,
    pub swap_fee_amount: u64,
    pub interest_amount: u64,
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
pub struct ManagerFeesClaimed {
    pub market: Pubkey,
    pub manager: Pubkey,
    pub asset_mint: Pubkey,
    pub swap_fee_amount: u64,
    pub interest_fee_amount: u64,
    pub remaining_manager_swap_fee_liability: u64,
    pub remaining_manager_interest_fee_liability: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct ProtocolAuctionConfigUpdated {
    pub authority: Pubkey,
    pub lane: u8,
    pub accepted_mint: Pubkey,
    pub start_multiplier_bps: u16,
    pub floor_multiplier_bps: u16,
    pub duration_slots: u64,
    pub max_reference_age_slots: u64,
    pub signer: Pubkey,
}

#[event]
pub struct ProtocolAuctionRecipientsUpdated {
    pub authority: Pubkey,
    pub lane: u8,
    pub treasury: Pubkey,
    pub staking_vault: Pubkey,
    pub treasury_bps: u16,
    pub staking_vault_bps: u16,
    pub signer: Pubkey,
}

#[event]
pub struct ProtocolAuctionSplitUpdated {
    pub authority: Pubkey,
    pub fee_auction_bps: u16,
    pub buyback_auction_bps: u16,
    pub signer: Pubkey,
}

#[event]
pub struct ProtocolAuctionSettled {
    pub market: Pubkey,
    pub reference_market: Pubkey,
    pub lane: u8,
    pub side: u8,
    pub bidder: Pubkey,
    pub sold_mint: Pubkey,
    pub accepted_mint: Pubkey,
    pub sold_amount: u64,
    pub payment_amount: u64,
    pub treasury_amount: u64,
    pub staking_vault_amount: u64,
    pub reference_price_nad: u64,
    pub auction_price_nad: u64,
    pub remaining_fee_liability: u64,
    pub remaining_buyback_liability: u64,
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
    pub base_hlp_pending_rebalance: i128,
    pub quote_hlp_pending_rebalance: i128,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct SwapSettled {
    pub market: Pubkey,
    pub trader: Pubkey,
    pub asset_in_side: u8,
    pub reserve_credit: u64,
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub base_hlp_pending_rebalance: i128,
    pub quote_hlp_pending_rebalance: i128,
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
pub struct PositionLiquidated {
    pub market: Pubkey,
    pub borrower: Pubkey,
    pub liquidator: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub collateral_asset_mint: Pubkey,
    pub repaid_amount: u64,
    pub collateral_seized: u64,
    pub collateral_to_liquidator: u64,
    pub insurance_funded: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub remaining_debt: u128,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct LiquidationAuctionOpened {
    pub market: Pubkey,
    pub margin_position: Pubkey,
    pub borrower: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub collateral_asset_mint: Pubkey,
    pub start_slot: u64,
    pub end_slot: u64,
    pub start_health_bps: u64,
    pub start_incentive_bps: u16,
    pub max_incentive_bps: u16,
    pub max_repay_amount: u64,
    pub reference_price_nad: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct LiquidationAuctionSettled {
    pub market: Pubkey,
    pub margin_position: Pubkey,
    pub borrower: Pubkey,
    pub bidder: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub collateral_asset_mint: Pubkey,
    pub repaid_amount: u64,
    pub collateral_to_bidder: u64,
    pub collateral_seized: u64,
    pub insurance_funded: u64,
    pub insurance_drawn: u64,
    pub socialized_loss: u64,
    pub auction_incentive_bps: u16,
    pub max_repay_amount: u64,
    pub remaining_debt: u128,
    pub auction_active: bool,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct HlpOpened {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub deposit_amount: u64,
    pub borrowed_amount: u64,
    pub ylp_amount: u64,
    pub hlp_amount: u64,
    pub hlp_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct HlpClosed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub asset_mint: Pubkey,
    pub hlp_amount: u64,
    pub ylp_amount: u64,
    pub target_amount_out: u64,
    pub debt_repaid: u64,
    pub interest_paid: u64,
    pub hlp_supply: u64,
    pub metadata: MarketEventMetadata,
}

#[event]
pub struct HlpRebalanced {
    pub market: Pubkey,
    pub target_side: u8,
    pub ideal_delta: i128,
    pub executed_delta: i128,
    pub pending_rebalance: i128,
    pub nav_nad: u128,
    pub metadata: MarketEventMetadata,
}
