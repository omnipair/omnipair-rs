use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{MarginPosition, Market, MarketAsset},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollateralReceipt {
    pub collateral_credit: u64,
    pub collateral_debit: u64,
    pub base_collateral: u64,
    pub quote_collateral: u64,
}

pub struct DepositCollateral {
    pub market_asset: MarketAsset,
    pub collateral_credit: u64,
}

pub struct WithdrawCollateral {
    pub market_asset: MarketAsset,
    pub collateral_debit: u64,
}

impl DepositCollateral {
    pub fn new(market_asset: MarketAsset, collateral_credit: u64) -> Self {
        Self {
            market_asset,
            collateral_credit,
        }
    }

    pub fn apply(self, margin_position: &mut MarginPosition) -> Result<CollateralReceipt> {
        require!(self.collateral_credit > 0, ErrorCode::AmountZero);
        match self.market_asset {
            MarketAsset::Base => {
                margin_position.base_collateral = margin_position
                    .base_collateral
                    .checked_add(self.collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                margin_position.quote_collateral = margin_position
                    .quote_collateral
                    .checked_add(self.collateral_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }

        Ok(CollateralReceipt {
            collateral_credit: self.collateral_credit,
            collateral_debit: 0,
            base_collateral: margin_position.base_collateral,
            quote_collateral: margin_position.quote_collateral,
        })
    }
}

impl WithdrawCollateral {
    pub fn new(market_asset: MarketAsset, collateral_debit: u64) -> Self {
        Self {
            market_asset,
            collateral_debit,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        margin_position: &mut MarginPosition,
    ) -> Result<CollateralReceipt> {
        require!(self.collateral_debit > 0, ErrorCode::AmountZero);
        market.enforce_daily_withdraw_limit(self.market_asset, self.collateral_debit)?;
        match self.market_asset {
            MarketAsset::Base => {
                require_gte!(
                    margin_position.idle_base_collateral()?,
                    self.collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.base_collateral = margin_position
                    .base_collateral
                    .checked_sub(self.collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                require_gte!(
                    margin_position.idle_quote_collateral()?,
                    self.collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.quote_collateral = margin_position
                    .quote_collateral
                    .checked_sub(self.collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        market.refresh_market_health()?;
        market.assert_risk_circuit_breakers()?;

        Ok(CollateralReceipt {
            collateral_credit: 0,
            collateral_debit: self.collateral_debit,
            base_collateral: margin_position.base_collateral,
            quote_collateral: margin_position.quote_collateral,
        })
    }
}
