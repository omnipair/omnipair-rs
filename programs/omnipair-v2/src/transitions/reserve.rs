use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, state::MarketSide};

pub struct AddLiquidity {
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
}

pub struct AddLiquidityReceipt {
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub base_ylp_supply: u64,
    pub quote_ylp_supply: u64,
}

pub struct RemoveLiquidity {
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
}

pub struct RemoveLiquidityReceipt {
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub base_ylp_supply: u64,
    pub quote_ylp_supply: u64,
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

        let base_ylp_amount = base_side
            .shares
            .shares_for_deposit(base_reserve_before, self.base_reserve_credit)?;
        let quote_ylp_amount = quote_side
            .shares
            .shares_for_deposit(quote_reserve_before, self.quote_reserve_credit)?;

        credit_reserve(base_side, self.base_reserve_credit, true)?;
        credit_reserve(quote_side, self.quote_reserve_credit, true)?;
        base_side.shares.mint(base_ylp_amount)?;
        quote_side.shares.mint(quote_ylp_amount)?;
        base_side.assert_share_backing()?;
        quote_side.assert_share_backing()?;

        Ok(AddLiquidityReceipt {
            base_reserve_credit: self.base_reserve_credit,
            quote_reserve_credit: self.quote_reserve_credit,
            base_ylp_amount,
            quote_ylp_amount,
            base_ylp_supply: base_side.shares.ylp_supply,
            quote_ylp_supply: quote_side.shares.ylp_supply,
        })
    }
}

impl RemoveLiquidity {
    pub fn new(base_ylp_amount: u64, quote_ylp_amount: u64) -> Self {
        Self {
            base_ylp_amount,
            quote_ylp_amount,
        }
    }

    pub fn apply(
        self,
        base_side: &mut MarketSide,
        quote_side: &mut MarketSide,
    ) -> Result<RemoveLiquidityReceipt> {
        require!(
            self.base_ylp_amount > 0 && self.quote_ylp_amount > 0,
            ErrorCode::AmountZero
        );
        let lhs = (self.base_ylp_amount as u128)
            .checked_mul(quote_side.shares.ylp_supply as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let rhs = (self.quote_ylp_amount as u128)
            .checked_mul(base_side.shares.ylp_supply as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_eq!(lhs, rhs, ErrorCode::SlippageExceeded);

        let base_amount_out = base_side
            .shares
            .reserve_for_burn(base_side.reserves.live_reserve, self.base_ylp_amount)?;
        let quote_amount_out = quote_side
            .shares
            .reserve_for_burn(quote_side.reserves.live_reserve, self.quote_ylp_amount)?;
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
        base_side.shares.burn(self.base_ylp_amount)?;
        quote_side.shares.burn(self.quote_ylp_amount)?;
        base_side.assert_share_backing()?;
        quote_side.assert_share_backing()?;

        Ok(RemoveLiquidityReceipt {
            base_ylp_amount: self.base_ylp_amount,
            quote_ylp_amount: self.quote_ylp_amount,
            base_amount_out,
            quote_amount_out,
            base_ylp_supply: base_side.shares.ylp_supply,
            quote_ylp_supply: quote_side.shares.ylp_supply,
        })
    }
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
    use super::*;
    use crate::state::MarketSide;

    #[test]
    fn add_liquidity_mints_floating_ylp_shares() {
        let mut base_side = MarketSide::default();
        let mut quote_side = MarketSide::default();

        let receipt = AddLiquidity::new(1_000_000, 2_000_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        assert_eq!(receipt.base_ylp_amount, 1_000_000);
        assert_eq!(receipt.quote_ylp_amount, 2_000_000);
        assert_eq!(base_side.shares.ylp_supply, 1_000_000);
        assert_eq!(quote_side.shares.ylp_supply, 2_000_000);
    }

    #[test]
    fn remove_liquidity_burns_matched_proportions() {
        let mut base_side = MarketSide::default();
        let mut quote_side = MarketSide::default();
        AddLiquidity::new(1_000_000, 2_000_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        let receipt = RemoveLiquidity::new(250_000, 500_000)
            .apply(&mut base_side, &mut quote_side)
            .unwrap();

        assert_eq!(receipt.base_amount_out, 250_000);
        assert_eq!(receipt.quote_amount_out, 500_000);
        assert_eq!(receipt.base_ylp_supply, 750_000);
        assert_eq!(receipt.quote_ylp_supply, 1_500_000);
    }
}
