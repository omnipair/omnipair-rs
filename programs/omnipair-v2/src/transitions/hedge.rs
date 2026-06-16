use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{HedgePosition, Market},
    transitions::fee::CarryForwardHedgedFees,
};

pub struct OpenHedge {
    pub market_side_index: u8,
    pub claim_credit: u64,
}

pub struct CloseHedge {
    pub market_side_index: u8,
    pub hedge_amount: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HedgeReceipt {
    pub claim_amount: u64,
    pub hedge_amount: u64,
    pub hedged_claim_token_supply: u64,
    pub accrued_fee_amount: u64,
}

impl OpenHedge {
    pub fn new(market_side_index: u8, claim_credit: u64) -> Self {
        Self {
            market_side_index,
            claim_credit,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        hedge_position: &mut HedgePosition,
    ) -> Result<HedgeReceipt> {
        require!(self.claim_credit > 0, ErrorCode::AmountZero);
        let (hedged_claim_token_supply, accrued_fee_amount) = {
            let market_side = market.side_mut(self.market_side_index)?;
            CarryForwardHedgedFees.apply(market_side)?;
            hedge_position.accrue_fees(market_side.fee_ledger.hedged_fee_growth_index_nad)?;
            market_side.claim_token_ledger.hedged_claim_token_supply = market_side
                .claim_token_ledger
                .hedged_claim_token_supply
                .checked_add(self.claim_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            hedge_position.increase(self.claim_credit)?;
            (
                market_side.claim_token_ledger.hedged_claim_token_supply,
                hedge_position.accrued_fee_amount,
            )
        };
        market.refresh_market_health()?;

        Ok(HedgeReceipt {
            claim_amount: self.claim_credit,
            hedge_amount: self.claim_credit,
            hedged_claim_token_supply,
            accrued_fee_amount,
        })
    }
}

impl CloseHedge {
    pub fn new(market_side_index: u8, hedge_amount: u64) -> Self {
        Self {
            market_side_index,
            hedge_amount,
        }
    }

    pub fn apply(
        self,
        market: &mut Market,
        hedge_position: &mut HedgePosition,
    ) -> Result<HedgeReceipt> {
        require!(self.hedge_amount > 0, ErrorCode::AmountZero);
        let (hedged_claim_token_supply, accrued_fee_amount) = {
            let market_side = market.side_mut(self.market_side_index)?;
            CarryForwardHedgedFees.apply(market_side)?;
            hedge_position.accrue_fees(market_side.fee_ledger.hedged_fee_growth_index_nad)?;
            market_side.claim_token_ledger.hedged_claim_token_supply = market_side
                .claim_token_ledger
                .hedged_claim_token_supply
                .checked_sub(self.hedge_amount)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            hedge_position.decrease(self.hedge_amount)?;
            (
                market_side.claim_token_ledger.hedged_claim_token_supply,
                hedge_position.accrued_fee_amount,
            )
        };
        market.refresh_market_health()?;

        Ok(HedgeReceipt {
            claim_amount: self.hedge_amount,
            hedge_amount: self.hedge_amount,
            hedged_claim_token_supply,
            accrued_fee_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{MARKET_VERSION, NAD},
        state::{ClaimTokenLedger, MarketConfig, MarketSide, ReserveLedger},
    };

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
                live_reserve: 1_000,
                cash_reserve: 1_000,
                reserved_liability: 0,
            },
            ..MarketSide::default()
        }
    }

    fn test_market() -> Market {
        let asset0_mint = Pubkey::new_unique();
        let asset1_mint = Pubkey::new_unique();
        let market = Market::initialize(
            asset0_mint,
            asset1_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(asset0_mint),
            market_side(asset1_mint),
            MarketConfig {
                swap_fee_bps: 30,
                operator_fee_bps: 1_000,
                buffer_ratio_bps: 2_000,
                fee_routing_k_nad: NAD,
                ema_half_life_ms: 60_000,
                directional_ema_half_life_ms: 60_000,
                k_ema_half_life_ms: 60_000,
                max_daily_borrow_bps: 2_000,
                max_daily_withdraw_bps: 2_000,
                spot_ema_divergence_bps: 1_000,
                k_ema_drawdown_bps: 1_000,
                recognized_collateral_cap_bps: 15_000,
                market_health_min_bps: 11_000,
                effective_debt_weight_min_bps: 10_000,
                effective_debt_gamma_nad: NAD,
                soft_borrow_enabled: false,
                hedged_lp_enabled: true,
                start_time: 0,
            },
            [11_u8; 32],
            42,
            253,
        )
        .unwrap();
        assert_eq!(market.version, MARKET_VERSION);
        market
    }

    fn hedge_position() -> HedgePosition {
        HedgePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_token_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    #[test]
    fn open_hedge_updates_position_and_side_supply() {
        let mut market = test_market();
        let mut hedge_position = hedge_position();

        let receipt = OpenHedge::new(0, 500)
            .apply(&mut market, &mut hedge_position)
            .unwrap();

        assert_eq!(receipt.claim_amount, 500);
        assert_eq!(receipt.hedge_amount, 500);
        assert_eq!(receipt.hedged_claim_token_supply, 500);
        assert_eq!(hedge_position.hedged_claim_token_amount, 500);
        assert_eq!(
            market.side0.claim_token_ledger.hedged_claim_token_supply,
            500
        );
    }

    #[test]
    fn close_hedge_updates_position_and_side_supply() {
        let mut market = test_market();
        market.side0.claim_token_ledger = ClaimTokenLedger {
            hedged_claim_token_supply: 500,
            ..ClaimTokenLedger::default()
        };
        let mut hedge_position = hedge_position();
        hedge_position.hedged_claim_token_amount = 500;

        let receipt = CloseHedge::new(0, 125)
            .apply(&mut market, &mut hedge_position)
            .unwrap();

        assert_eq!(receipt.claim_amount, 125);
        assert_eq!(receipt.hedge_amount, 125);
        assert_eq!(receipt.hedged_claim_token_supply, 375);
        assert_eq!(hedge_position.hedged_claim_token_amount, 375);
        assert_eq!(
            market.side0.claim_token_ledger.hedged_claim_token_supply,
            375
        );
    }
}
