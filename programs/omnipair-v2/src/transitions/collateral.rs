use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{MarginPosition, Market},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollateralReceipt {
    pub collateral_credit: u64,
    pub collateral_debit: u64,
    pub collateral0: u64,
    pub collateral1: u64,
}

pub struct DepositCollateral {
    pub market_side_index: u8,
    pub collateral_credit: u64,
}

pub struct WithdrawCollateral {
    pub market_side_index: u8,
    pub collateral_debit: u64,
}

impl DepositCollateral {
    pub fn new(market_side_index: u8, collateral_credit: u64) -> Self {
        Self {
            market_side_index,
            collateral_credit,
        }
    }

    pub fn apply(self, margin_position: &mut MarginPosition) -> Result<CollateralReceipt> {
        require!(self.collateral_credit > 0, ErrorCode::AmountZero);
        match self.market_side_index {
            0 => {
                margin_position.collateral0 = margin_position
                    .collateral0
                    .checked_add(self.collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            1 => {
                margin_position.collateral1 = margin_position
                    .collateral1
                    .checked_add(self.collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            _ => return err!(ErrorCode::InvalidMarketSide),
        }

        Ok(CollateralReceipt {
            collateral_credit: self.collateral_credit,
            collateral_debit: 0,
            collateral0: margin_position.collateral0,
            collateral1: margin_position.collateral1,
        })
    }
}

impl WithdrawCollateral {
    pub fn new(market_side_index: u8, collateral_debit: u64) -> Self {
        Self {
            market_side_index,
            collateral_debit,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<CollateralReceipt> {
        require!(self.collateral_debit > 0, ErrorCode::AmountZero);
        market.enforce_daily_withdraw_limit(self.market_side_index, self.collateral_debit)?;
        match self.market_side_index {
            0 => {
                require_gte!(
                    margin_position.idle_collateral0()?,
                    self.collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.collateral0 = margin_position
                    .collateral0
                    .checked_sub(self.collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            1 => {
                require_gte!(
                    margin_position.idle_collateral1()?,
                    self.collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.collateral1 = margin_position
                    .collateral1
                    .checked_sub(self.collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            _ => return err!(ErrorCode::InvalidMarketSide),
        }
        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;

        Ok(CollateralReceipt {
            collateral_credit: 0,
            collateral_debit: self.collateral_debit,
            collateral0: margin_position.collateral0,
            collateral1: margin_position.collateral1,
        })
    }
}
