use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::MarketSide,
    transitions::fee::{FeeLedgerReceipt, RecordFeeCredit},
    utils::market_math::require_market_reserve_floor,
};

pub struct Swap {
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub operator_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub fee_routing_k_nad: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwapReceipt {
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub reserve_in_live_reserve: u64,
    pub reserve_out_live_reserve: u64,
    pub fee_ledger: FeeLedgerReceipt,
}

impl Swap {
    pub fn new(
        amount_in_after_fee: u64,
        amount_out: u64,
        fee_credit: u64,
        operator_fee_bps: u16,
        protocol_fee_bps: u16,
        fee_routing_k_nad: u64,
    ) -> Self {
        Self {
            amount_in_after_fee,
            amount_out,
            fee_credit,
            operator_fee_bps,
            protocol_fee_bps,
            fee_routing_k_nad,
        }
    }

    pub fn apply(
        self,
        market_side_in: &mut MarketSide,
        market_side_out: &mut MarketSide,
    ) -> Result<SwapReceipt> {
        require_gte!(
            market_side_out.reserve_ledger.cash_reserve,
            self.amount_out,
            ErrorCode::InsufficientMarketClaimCoverage
        );
        let next_out_reserve = market_side_out
            .reserve_ledger
            .live_reserve
            .checked_sub(self.amount_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        require_market_reserve_floor(
            next_out_reserve,
            market_side_out
                .claim_token_ledger
                .protected_claim_token_supply,
            market_side_out.buffer_ledger.required_buffer,
        )?;

        market_side_in.reserve_ledger.live_reserve = market_side_in
            .reserve_ledger
            .live_reserve
            .checked_add(self.amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_in.reserve_ledger.cash_reserve = market_side_in
            .reserve_ledger
            .cash_reserve
            .checked_add(self.amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_out.reserve_ledger.live_reserve = next_out_reserve;
        market_side_out.reserve_ledger.cash_reserve = market_side_out
            .reserve_ledger
            .cash_reserve
            .checked_sub(self.amount_out)
            .ok_or(ErrorCode::CashReserveUnderflow)?;
        let fee_ledger = RecordFeeCredit::new(
            self.fee_credit,
            self.operator_fee_bps,
            self.protocol_fee_bps,
            self.fee_routing_k_nad,
        )
        .apply(market_side_in)?;
        market_side_in.assert_claim_coverage()?;
        market_side_out.assert_claim_coverage()?;
        market_side_in.fee_ledger.assert_backed()?;

        Ok(SwapReceipt {
            amount_in_after_fee: self.amount_in_after_fee,
            amount_out: self.amount_out,
            fee_credit: self.fee_credit,
            reserve_in_live_reserve: market_side_in.reserve_ledger.live_reserve,
            reserve_out_live_reserve: market_side_out.reserve_ledger.live_reserve,
            fee_ledger,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constants::NAD, state::MarketSide};

    fn market_side(
        live_reserve: u64,
        cash_reserve: u64,
        protected_claim_token_supply: u64,
        required_buffer: u64,
    ) -> MarketSide {
        MarketSide {
            asset_mint: Pubkey::new_unique(),
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: crate::state::ReserveLedger {
                live_reserve,
                cash_reserve,
                reserved_liability: 0,
            },
            claim_token_ledger: crate::state::ClaimTokenLedger {
                protected_claim_token_supply,
                ..crate::state::ClaimTokenLedger::default()
            },
            buffer_ledger: crate::state::BufferLedger {
                required_buffer,
                buffer_ratio_bps: 2_000,
                ..crate::state::BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    #[test]
    fn swap_enforces_output_market_reserve_floor() {
        let mut market_side_in = market_side(10_000, 10_000, 8_000, 2_000);
        let mut market_side_out = market_side(12_000, 12_000, 8_000, 2_000);

        Swap::new(500, 2_000, 0, 0, 0, NAD)
            .apply(&mut market_side_in, &mut market_side_out)
            .unwrap();
        assert_eq!(market_side_out.reserve_ledger.live_reserve, 10_000);
        assert_eq!(market_side_out.reserve_ledger.cash_reserve, 10_000);

        let mut market_side_in = market_side(10_000, 10_000, 8_000, 2_000);
        let mut market_side_out = market_side(12_000, 12_000, 8_000, 2_000);
        let err = Swap::new(500, 2_001, 0, 0, 0, NAD)
            .apply(&mut market_side_in, &mut market_side_out)
            .unwrap_err();
        assert_eq!(err, error!(ErrorCode::InsufficientMarketClaimCoverage));
    }

    #[test]
    fn swap_routes_non_compounding_fee_liabilities() {
        let mut market_side_in = market_side(10_000, 10_000, 8_000, 2_000);
        market_side_in.claim_token_ledger.staked_claim_token_supply = 8_000;
        market_side_in.buffer_ledger.staked_buffer_share_amount = 2_000;
        let mut market_side_out = market_side(12_000, 12_000, 8_000, 2_000);

        let receipt = Swap::new(500, 100, 1_000, 1_000, 0, NAD)
            .apply(&mut market_side_in, &mut market_side_out)
            .unwrap();

        assert_eq!(receipt.reserve_in_live_reserve, 10_500);
        assert_eq!(market_side_in.reserve_ledger.live_reserve, 10_500);
        assert_eq!(market_side_in.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side_in.fee_ledger.operator_fee_liability, 100);
        assert_eq!(market_side_in.fee_ledger.fee_liability, 900);
        assert_eq!(market_side_in.fee_ledger.unallocated_fee_liability, 0);
        assert_eq!(market_side_in.fee_ledger.fee_growth_index_nad, 90_000_000);
        market_side_in.fee_ledger.assert_backed().unwrap();
    }
}
