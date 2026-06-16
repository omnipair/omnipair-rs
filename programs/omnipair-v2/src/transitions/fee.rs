use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, NAD, TAYLOR_TERMS},
    errors::ErrorCode,
    shared::math::taylor_exp,
    state::MarketSide,
    utils::market_math::active_stake_units,
};

pub struct RecordFeeCredit {
    pub fee_credit: u64,
    pub operator_fee_bps: u16,
    pub fee_routing_k_nad: u64,
}

pub struct CarryForwardStakerFees;

pub struct CarryForwardHedgedFees;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeeLedgerReceipt {
    pub fee_growth_index_nad: u128,
    pub hedged_fee_growth_index_nad: u128,
    pub fee_liability: u64,
    pub hedged_fee_liability: u64,
    pub unallocated_fee_liability: u64,
    pub unallocated_hedged_fee_liability: u64,
    pub operator_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub fee_vault_balance: u64,
}

impl FeeLedgerReceipt {
    pub fn from_side(market_side: &MarketSide) -> Self {
        let ledger = &market_side.fee_ledger;
        Self {
            fee_growth_index_nad: ledger.fee_growth_index_nad,
            hedged_fee_growth_index_nad: ledger.hedged_fee_growth_index_nad,
            fee_liability: ledger.fee_liability,
            hedged_fee_liability: ledger.hedged_fee_liability,
            unallocated_fee_liability: ledger.unallocated_fee_liability,
            unallocated_hedged_fee_liability: ledger.unallocated_hedged_fee_liability,
            operator_fee_liability: ledger.operator_fee_liability,
            protocol_fee_liability: ledger.protocol_fee_liability,
            fee_vault_balance: ledger.fee_vault_balance,
        }
    }
}

impl RecordFeeCredit {
    pub fn new(fee_credit: u64, operator_fee_bps: u16, fee_routing_k_nad: u64) -> Self {
        Self {
            fee_credit,
            operator_fee_bps,
            fee_routing_k_nad,
        }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        if self.fee_credit == 0 {
            return Ok(FeeLedgerReceipt::from_side(market_side));
        }
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require!(self.fee_routing_k_nad > 0, ErrorCode::InvalidMarketConfig);

        let operator_fee = (self.fee_credit as u128)
            .checked_mul(self.operator_fee_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let operator_fee =
            u64::try_from(operator_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
        let lp_fee = self
            .fee_credit
            .checked_sub(operator_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;

        market_side.fee_ledger.fee_vault_balance = market_side
            .fee_ledger
            .fee_vault_balance
            .checked_add(self.fee_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fee_ledger.operator_fee_liability = market_side
            .fee_ledger
            .operator_fee_liability
            .checked_add(operator_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;

        let (free_lp_fee, routed_fee) = routed_lp_fee(market_side, lp_fee, self.fee_routing_k_nad)?;
        record_hedged_fee_credit(market_side, routed_fee)?;

        let active_units = active_stake_units(
            market_side.claim_token_ledger.staked_claim_token_supply,
            market_side.buffer_ledger.staked_buffer_share_amount,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        if free_lp_fee > 0 {
            market_side.fee_ledger.unallocated_fee_liability = market_side
                .fee_ledger
                .unallocated_fee_liability
                .checked_add(free_lp_fee)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }

        carry_forward_unallocated_fee_with_units(market_side, active_units)?;
        market_side.fee_ledger.assert_backed()?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

impl CarryForwardStakerFees {
    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        let active_units = active_stake_units(
            market_side.claim_token_ledger.staked_claim_token_supply,
            market_side.buffer_ledger.staked_buffer_share_amount,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        carry_forward_unallocated_fee_with_units(market_side, active_units)?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

impl CarryForwardHedgedFees {
    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        carry_forward_unallocated_hedged_fee_with_supply(
            market_side,
            market_side.claim_token_ledger.hedged_claim_token_supply,
        )?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

fn carry_forward_unallocated_fee_with_units(
    market_side: &mut MarketSide,
    active_units: u64,
) -> Result<()> {
    if active_units == 0 || market_side.fee_ledger.unallocated_fee_liability == 0 {
        return Ok(());
    }

    let fee_amount = market_side.fee_ledger.unallocated_fee_liability;
    market_side.fee_ledger.unallocated_fee_liability = 0;
    let index_delta = (fee_amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(active_units as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = index_delta
        .checked_mul(active_units as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = u64::try_from(allocated_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let unallocated_fee = fee_amount
        .checked_sub(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    market_side.fee_ledger.fee_growth_index_nad = market_side
        .fee_ledger
        .fee_growth_index_nad
        .checked_add(index_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.fee_liability = market_side
        .fee_ledger
        .fee_liability
        .checked_add(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.unallocated_fee_liability = market_side
        .fee_ledger
        .unallocated_fee_liability
        .checked_add(unallocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

fn record_hedged_fee_credit(market_side: &mut MarketSide, fee_amount: u64) -> Result<()> {
    if fee_amount == 0 {
        return Ok(());
    }
    market_side.fee_ledger.unallocated_hedged_fee_liability = market_side
        .fee_ledger
        .unallocated_hedged_fee_liability
        .checked_add(fee_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    carry_forward_unallocated_hedged_fee_with_supply(
        market_side,
        market_side.claim_token_ledger.hedged_claim_token_supply,
    )
}

fn carry_forward_unallocated_hedged_fee_with_supply(
    market_side: &mut MarketSide,
    hedged_claim_token_supply: u64,
) -> Result<()> {
    if hedged_claim_token_supply == 0
        || market_side.fee_ledger.unallocated_hedged_fee_liability == 0
    {
        return Ok(());
    }

    let fee_amount = market_side.fee_ledger.unallocated_hedged_fee_liability;
    market_side.fee_ledger.unallocated_hedged_fee_liability = 0;
    let index_delta = (fee_amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(hedged_claim_token_supply as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = index_delta
        .checked_mul(hedged_claim_token_supply as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = u64::try_from(allocated_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let unallocated_fee = fee_amount
        .checked_sub(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    market_side.fee_ledger.hedged_fee_growth_index_nad = market_side
        .fee_ledger
        .hedged_fee_growth_index_nad
        .checked_add(index_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.hedged_fee_liability = market_side
        .fee_ledger
        .hedged_fee_liability
        .checked_add(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.unallocated_hedged_fee_liability = market_side
        .fee_ledger
        .unallocated_hedged_fee_liability
        .checked_add(unallocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

fn routed_lp_fee(
    market_side: &MarketSide,
    lp_fee: u64,
    fee_routing_k_nad: u64,
) -> Result<(u64, u64)> {
    if lp_fee == 0 || market_side.claim_token_ledger.hedged_claim_token_supply == 0 {
        return Ok((lp_fee, 0));
    }
    let free_buffer = market_side.free_buffer()?;
    if free_buffer == 0 {
        return Ok((lp_fee, 0));
    }
    let eta_nad = (market_side.claim_token_ledger.hedged_claim_token_supply as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(free_buffer as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let free_share_nad = dynamic_free_fee_share_nad(eta_nad, fee_routing_k_nad)?;
    let free_lp_fee = (lp_fee as u128)
        .checked_mul(free_share_nad)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let free_lp_fee = u64::try_from(free_lp_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let routed_fee = lp_fee
        .checked_sub(free_lp_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((free_lp_fee, routed_fee))
}

fn dynamic_free_fee_share_nad(eta_nad: u128, fee_routing_k_nad: u64) -> Result<u128> {
    require!(fee_routing_k_nad > 0, ErrorCode::InvalidMarketConfig);
    if eta_nad == 0 {
        return Ok(0);
    }
    let x_nad = eta_nad
        .checked_mul(fee_routing_k_nad as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .unwrap_or(i64::MAX as u128)
        .min(i64::MAX as u128) as i64;
    let hedged_share_nad = taylor_exp(-x_nad, NAD, TAYLOR_TERMS) as u128;
    Ok((NAD as u128).saturating_sub(hedged_share_nad))
}
