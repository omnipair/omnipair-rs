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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::{MarketConfig, MarketSide, ReserveLedger},
    };

    const TEST_RESERVE: u64 = 1_000_000_000;

    fn market_side(asset_mint: Pubkey) -> MarketSide {
        MarketSide {
            asset_mint,
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: ReserveLedger {
                live_reserve: TEST_RESERVE,
                cash_reserve: TEST_RESERVE,
                reserved_liability: 0,
            },
            ..MarketSide::default()
        }
    }

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint),
            market_side(quote_mint),
            MarketConfig {
                swap_fee_bps: 30,
                operator_fee_bps: 1_000,
                protocol_fee_bps: 0,
                buffer_ratio_bps: 2_000,
                fee_routing_k_nad: NAD,
                ema_half_life_ms: 60_000,
                directional_ema_half_life_ms: 60_000,
                k_ema_half_life_ms: 60_000,
                max_daily_borrow_bps: BPS_DENOMINATOR,
                max_daily_withdraw_bps: BPS_DENOMINATOR,
                spot_ema_divergence_bps: BPS_DENOMINATOR,
                k_ema_drawdown_bps: BPS_DENOMINATOR,
                recognized_collateral_cap_bps: 20_000,
                market_health_min_bps: BPS_DENOMINATOR,
                effective_debt_weight_min_bps: BPS_DENOMINATOR,
                effective_debt_gamma_nad: NAD,
                soft_borrow_enabled: false,
                hedged_lp_enabled: true,
                start_time: 0,
            },
            [17_u8; 32],
            42,
            251,
        )
        .unwrap()
    }

    fn margin_position() -> MarginPosition {
        MarginPosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            base_collateral: 0,
            quote_collateral: 0,
            recognized_base_collateral_for_quote_debt: 0,
            recognized_quote_collateral_for_base_debt: 0,
            fixed_base_debt_shares: 0,
            fixed_quote_debt_shares: 0,
            bump: 1,
        }
    }

    #[test]
    fn deposit_collateral_credits_only_selected_side() {
        let mut position = margin_position();

        let base_receipt = DepositCollateral::new(MarketAsset::Base, 700)
            .apply(&mut position)
            .unwrap();
        let quote_receipt = DepositCollateral::new(MarketAsset::Quote, 300)
            .apply(&mut position)
            .unwrap();

        assert_eq!(base_receipt.collateral_credit, 700);
        assert_eq!(base_receipt.collateral_debit, 0);
        assert_eq!(base_receipt.base_collateral, 700);
        assert_eq!(base_receipt.quote_collateral, 0);
        assert_eq!(quote_receipt.collateral_credit, 300);
        assert_eq!(quote_receipt.base_collateral, 700);
        assert_eq!(quote_receipt.quote_collateral, 300);
        assert_eq!(position.base_collateral, 700);
        assert_eq!(position.quote_collateral, 300);
    }

    #[test]
    fn withdraw_collateral_debits_idle_collateral_and_records_daily_bucket() {
        let mut market = test_market();
        let mut position = margin_position();
        position.base_collateral = 900;
        position.recognized_base_collateral_for_quote_debt = 400;

        let receipt = WithdrawCollateral::new(MarketAsset::Base, 500)
            .apply(&mut market, &mut position)
            .unwrap();

        assert_eq!(receipt.collateral_credit, 0);
        assert_eq!(receipt.collateral_debit, 500);
        assert_eq!(receipt.base_collateral, 400);
        assert_eq!(receipt.quote_collateral, 0);
        assert_eq!(position.base_collateral, 400);
        assert_eq!(position.recognized_base_collateral_for_quote_debt, 400);
        assert_eq!(market.base_side.daily_limit_book.withdrawn_bucket, 500);
    }

    #[test]
    fn withdraw_collateral_rejects_recognized_collateral() {
        let mut market = test_market();
        let mut position = margin_position();
        position.quote_collateral = 900;
        position.recognized_quote_collateral_for_base_debt = 400;

        let err = WithdrawCollateral::new(MarketAsset::Quote, 501)
            .apply(&mut market, &mut position)
            .unwrap_err();

        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InsufficientRecognizedCollateral)
        );
        assert_eq!(position.quote_collateral, 900);
        assert_eq!(position.recognized_quote_collateral_for_base_debt, 400);
    }

    #[test]
    fn collateral_transitions_reject_zero_amounts() {
        let mut market = test_market();
        let mut position = margin_position();

        let deposit_err = DepositCollateral::new(MarketAsset::Base, 0)
            .apply(&mut position)
            .unwrap_err();
        let withdraw_err = WithdrawCollateral::new(MarketAsset::Base, 0)
            .apply(&mut market, &mut position)
            .unwrap_err();

        assert_eq!(
            deposit_err,
            anchor_lang::prelude::error!(ErrorCode::AmountZero)
        );
        assert_eq!(
            withdraw_err,
            anchor_lang::prelude::error!(ErrorCode::AmountZero)
        );
    }
}
