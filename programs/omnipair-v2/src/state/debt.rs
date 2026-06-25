use anchor_lang::prelude::*;

use crate::{constants::NAD, errors::ErrorCode, shared::math::ceil_div};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Debt {
    pub fixed_base_shares: u128,
    pub fixed_quote_shares: u128,
    pub soft_base_shares: u128,
    pub soft_quote_shares: u128,
    pub base_borrow_index_nad: u128,
    pub quote_borrow_index_nad: u128,
    pub base_rate_at_target_nad: u128,
    pub quote_rate_at_target_nad: u128,
    pub recognized_base_collateral_for_quote_debt: u64,
    pub recognized_quote_collateral_for_base_debt: u64,
    pub last_recognition_slot: u64,
    pub last_accrual_slot: u64,
}

impl Debt {
    pub fn debt_to_shares(amount: u64, borrow_index_nad: u128) -> Result<u128> {
        require!(amount > 0, ErrorCode::AmountZero);
        ceil_div(
            (amount as u128)
                .checked_mul(NAD as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            borrow_index_nad,
        )
        .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn shares_to_debt(shares: u128, borrow_index_nad: u128) -> Result<u128> {
        shares
            .checked_mul(borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn fixed_base_debt(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_base_shares, self.base_borrow_index_nad)
    }

    pub fn fixed_quote_debt(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_quote_shares, self.quote_borrow_index_nad)
    }

    pub fn soft_base_debt(&self) -> Result<u128> {
        self.soft_base_shares
            .checked_mul(self.base_borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn soft_quote_debt(&self) -> Result<u128> {
        self.soft_quote_shares
            .checked_mul(self.quote_borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_base_debt(&self) -> Result<u128> {
        self.fixed_base_debt()?
            .checked_add(self.soft_base_debt()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_quote_debt(&self) -> Result<u128> {
        self.fixed_quote_debt()?
            .checked_add(self.soft_quote_debt()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }
}
