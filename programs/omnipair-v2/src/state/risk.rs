use anchor_lang::prelude::*;

use crate::{constants::NAD, errors::ErrorCode, shared::math::ceil_div};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DebtBook {
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub soft_debt0_shares: u128,
    pub soft_debt1_shares: u128,
    pub borrow_index0_nad: u128,
    pub borrow_index1_nad: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RiskBook {
    pub price0_ema_nad: u64,
    pub price1_ema_nad: u64,
    pub directional_price0_ema_nad: u64,
    pub directional_price1_ema_nad: u64,
    pub cached_spot_price0_nad: u64,
    pub cached_spot_price1_nad: u64,
    pub cached_k_nad: u128,
    pub cached_liquidity_nad: u128,
    pub cached_liquidity0_nad: u128,
    pub cached_liquidity1_nad: u128,
    pub k_ema: u128,
    pub liquidity_ema: u128,
    pub liquidity0_ema: u128,
    pub liquidity1_ema: u128,
    pub last_snapshot_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketHealth {
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub effective_debt0_nad: u128,
    pub effective_debt1_nad: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
}

impl DebtBook {
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

    pub fn fixed_debt0(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_debt0_shares, self.borrow_index0_nad)
    }

    pub fn fixed_debt1(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_debt1_shares, self.borrow_index1_nad)
    }

    pub fn soft_debt0(&self) -> Result<u128> {
        self.soft_debt0_shares
            .checked_mul(self.borrow_index0_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn soft_debt1(&self) -> Result<u128> {
        self.soft_debt1_shares
            .checked_mul(self.borrow_index1_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_debt0(&self) -> Result<u128> {
        self.fixed_debt0()?
            .checked_add(self.soft_debt0()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn total_debt1(&self) -> Result<u128> {
        self.fixed_debt1()?
            .checked_add(self.soft_debt1()?)
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }
}
