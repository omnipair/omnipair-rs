use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::MarketSide,
    utils::market_math::{required_buffer_for_claims, split_claim_minus_buffer},
};

pub struct AddLiquidity {
    pub reserve_credit: u64,
}

pub struct AddLiquidityReceipt {
    pub reserve_credit: u64,
    pub claim_amount: u64,
    pub buffer_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
}

impl AddLiquidity {
    pub fn new(reserve_credit: u64) -> Self {
        Self { reserve_credit }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<AddLiquidityReceipt> {
        require!(self.reserve_credit > 0, ErrorCode::AmountZero);
        let (claim_amount, buffer_amount) = split_claim_minus_buffer(
            self.reserve_credit,
            market_side.buffer_book.buffer_ratio_bps,
        )?;
        require!(claim_amount > 0 && buffer_amount > 0, ErrorCode::AmountZero);

        let next_claim_supply = market_side
            .claim_ledger
            .protected_claim_supply
            .checked_add(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_buffer_shares = market_side
            .buffer_book
            .buffer_shares
            .checked_add(buffer_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_required_buffer = required_buffer_for_claims(
            next_claim_supply,
            market_side.buffer_book.buffer_ratio_bps,
        )?;
        require_gte!(
            next_buffer_shares,
            next_required_buffer,
            ErrorCode::InsufficientBufferShares
        );

        market_side.reserve_ledger.live_reserve = market_side
            .reserve_ledger
            .live_reserve
            .checked_add(self.reserve_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side.reserve_ledger.cash_reserve = market_side
            .reserve_ledger
            .cash_reserve
            .checked_add(self.reserve_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side.claim_ledger.protected_claim_supply = next_claim_supply;
        market_side.buffer_book.buffer_shares = next_buffer_shares;
        market_side.buffer_book.required_buffer = next_required_buffer;
        market_side.assert_claim_coverage()?;

        Ok(AddLiquidityReceipt {
            reserve_credit: self.reserve_credit,
            claim_amount,
            buffer_amount,
            protected_claim_supply: market_side.claim_ledger.protected_claim_supply,
            required_buffer: market_side.buffer_book.required_buffer,
        })
    }
}

pub struct RemoveLiquidity {
    pub claim_amount: u64,
}

pub struct RemoveLiquidityReceipt {
    pub claim_amount: u64,
    pub protected_claim_supply: u64,
    pub required_buffer: u64,
}

impl RemoveLiquidity {
    pub fn new(claim_amount: u64) -> Self {
        Self { claim_amount }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<RemoveLiquidityReceipt> {
        require!(self.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            market_side.claim_ledger.protected_claim_supply,
            self.claim_amount,
            ErrorCode::InsufficientMarketClaimCoverage
        );
        require_gte!(
            market_side.reserve_ledger.cash_reserve,
            self.claim_amount,
            ErrorCode::InsufficientMarketClaimCoverage
        );

        let next_claim_supply = market_side
            .claim_ledger
            .protected_claim_supply
            .checked_sub(self.claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_required_buffer = required_buffer_for_claims(
            next_claim_supply,
            market_side.buffer_book.buffer_ratio_bps,
        )?;
        let next_live_reserve = market_side
            .reserve_ledger
            .live_reserve
            .checked_sub(self.claim_amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        let reserve_floor = next_claim_supply
            .checked_add(next_required_buffer)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(
            next_live_reserve,
            reserve_floor,
            ErrorCode::InsufficientMarketClaimCoverage
        );

        market_side.reserve_ledger.live_reserve = next_live_reserve;
        market_side.reserve_ledger.cash_reserve = market_side
            .reserve_ledger
            .cash_reserve
            .checked_sub(self.claim_amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;
        market_side.claim_ledger.protected_claim_supply = next_claim_supply;
        market_side.buffer_book.required_buffer = next_required_buffer;

        Ok(RemoveLiquidityReceipt {
            claim_amount: self.claim_amount,
            protected_claim_supply: market_side.claim_ledger.protected_claim_supply,
            required_buffer: market_side.buffer_book.required_buffer,
        })
    }
}
