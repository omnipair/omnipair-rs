use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, state::MarketSide};

pub struct AddLiquidity {
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
}

pub struct AddLiquidityReceipt {
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
    pub ylp_amount: u64,
    pub ylp_supply: u64,
}

pub struct RemoveLiquidity {
    pub ylp_amount: u64,
}

pub struct RemoveLiquidityReceipt {
    pub ylp_amount: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub ylp_supply: u64,
}

impl AddLiquidity {
    pub fn new(base_reserve_credit: u64, quote_reserve_credit: u64) -> Self {
        Self {
            base_reserve_credit,
            quote_reserve_credit,
        }
    }

    pub fn apply(
        self,
        base_side: &mut MarketSide,
        quote_side: &mut MarketSide,
    ) -> Result<AddLiquidityReceipt> {
        require!(
            self.base_reserve_credit > 0 && self.quote_reserve_credit > 0,
            ErrorCode::AmountZero
        );
        let base_reserve_before = base_side.reserves.live_reserve;
        let quote_reserve_before = quote_side.reserves.live_reserve;
        if base_reserve_before > 0 || quote_reserve_before > 0 {
            require!(
                base_reserve_before > 0 && quote_reserve_before > 0,
                ErrorCode::InsufficientLiquidity
            );
            let lhs = (self.base_reserve_credit as u128)
                .checked_mul(quote_reserve_before as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            let rhs = (self.quote_reserve_credit as u128)
                .checked_mul(base_reserve_before as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            require_eq!(lhs, rhs, ErrorCode::SlippageExceeded);
        }

        let ylp_amount = market_ylp_for_deposit(
            base_side,
            quote_side,
            base_reserve_before,
            quote_reserve_before,
            self.base_reserve_credit,
            self.quote_reserve_credit,
        )?;
        require!(ylp_amount > 0, ErrorCode::SlippageExceeded);

        credit_reserve(base_side, self.base_reserve_credit, true)?;
        credit_reserve(quote_side, self.quote_reserve_credit, true)?;
        base_side.shares.mint(ylp_amount)?;
        quote_side.shares.mint(ylp_amount)?;
        base_side.assert_share_backing()?;
        quote_side.assert_share_backing()?;

        Ok(AddLiquidityReceipt {
            base_reserve_credit: self.base_reserve_credit,
            quote_reserve_credit: self.quote_reserve_credit,
            ylp_amount,
            ylp_supply: base_side.shares.ylp_supply,
        })
    }
}

impl RemoveLiquidity {
    pub fn new(ylp_amount: u64) -> Self {
        Self { ylp_amount }
    }

    pub fn apply(
        self,
        base_side: &mut MarketSide,
        quote_side: &mut MarketSide,
    ) -> Result<RemoveLiquidityReceipt> {
        require!(self.ylp_amount > 0, ErrorCode::AmountZero);
        require_eq!(
            base_side.shares.ylp_supply,
            quote_side.shares.ylp_supply,
            ErrorCode::BrokenInvariant
        );

        let base_amount_out = base_side
            .shares
            .reserve_for_burn(base_side.reserves.live_reserve, self.ylp_amount)?;
        let quote_amount_out = quote_side
            .shares
            .reserve_for_burn(quote_side.reserves.live_reserve, self.ylp_amount)?;
        require_gte!(
            base_side.reserves.cash_reserve,
            base_amount_out,
            ErrorCode::InsufficientLiquidity
        );
        require_gte!(
            quote_side.reserves.cash_reserve,
            quote_amount_out,
            ErrorCode::InsufficientLiquidity
        );

        debit_reserve(base_side, base_amount_out, true)?;
        debit_reserve(quote_side, quote_amount_out, true)?;
        base_side.shares.burn(self.ylp_amount)?;
        quote_side.shares.burn(self.ylp_amount)?;
        base_side.assert_share_backing()?;
        quote_side.assert_share_backing()?;

        Ok(RemoveLiquidityReceipt {
            ylp_amount: self.ylp_amount,
            base_amount_out,
            quote_amount_out,
            ylp_supply: base_side.shares.ylp_supply,
        })
    }
}

pub fn market_ylp_for_deposit(
    base_side: &MarketSide,
    quote_side: &MarketSide,
    base_reserve_before: u64,
    quote_reserve_before: u64,
    base_amount: u64,
    quote_amount: u64,
) -> Result<u64> {
    require_eq!(
        base_side.shares.ylp_supply,
        quote_side.shares.ylp_supply,
        ErrorCode::BrokenInvariant
    );
    if base_side.shares.ylp_supply == 0 {
        return Ok(base_amount);
    }
    let base_ylp = base_side
        .shares
        .shares_for_deposit(base_reserve_before, base_amount)?;
    let quote_ylp = quote_side
        .shares
        .shares_for_deposit(quote_reserve_before, quote_amount)?;
    Ok(base_ylp.min(quote_ylp))
}

pub fn credit_reserve(market_side: &mut MarketSide, amount: u64, credit_cash: bool) -> Result<()> {
    market_side.reserves.live_reserve = market_side
        .reserves
        .live_reserve
        .checked_add(amount)
        .ok_or(ErrorCode::ReserveOverflow)?;
    if credit_cash {
        market_side.reserves.cash_reserve = market_side
            .reserves
            .cash_reserve
            .checked_add(amount)
            .ok_or(ErrorCode::ReserveOverflow)?;
    }
    Ok(())
}

pub fn debit_reserve(market_side: &mut MarketSide, amount: u64, debit_cash: bool) -> Result<()> {
    market_side.reserves.live_reserve = market_side
        .reserves
        .live_reserve
        .checked_sub(amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    if debit_cash {
        market_side.reserves.cash_reserve = market_side
            .reserves
            .cash_reserve
            .checked_sub(amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("../tests/transitions/reserve.rs");
}
