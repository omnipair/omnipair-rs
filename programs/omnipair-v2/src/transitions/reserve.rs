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
    pub protected_claim_token_supply: u64,
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
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        require!(claim_amount > 0 && buffer_amount > 0, ErrorCode::AmountZero);

        let next_claim_supply = market_side
            .claim_token_ledger
            .protected_claim_token_supply
            .checked_add(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_buffer_share_supply = market_side
            .buffer_ledger
            .buffer_share_supply
            .checked_add(buffer_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_required_buffer = required_buffer_for_claims(
            next_claim_supply,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        require_gte!(
            next_buffer_share_supply,
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
        market_side.claim_token_ledger.protected_claim_token_supply = next_claim_supply;
        market_side.buffer_ledger.buffer_share_supply = next_buffer_share_supply;
        market_side.buffer_ledger.required_buffer = next_required_buffer;
        market_side.assert_claim_coverage()?;

        Ok(AddLiquidityReceipt {
            reserve_credit: self.reserve_credit,
            claim_amount,
            buffer_amount,
            protected_claim_token_supply: market_side
                .claim_token_ledger
                .protected_claim_token_supply,
            required_buffer: market_side.buffer_ledger.required_buffer,
        })
    }
}

pub struct RemoveLiquidity {
    pub claim_amount: u64,
}

pub struct RemoveLiquidityReceipt {
    pub claim_amount: u64,
    pub protected_claim_token_supply: u64,
    pub required_buffer: u64,
}

impl RemoveLiquidity {
    pub fn new(claim_amount: u64) -> Self {
        Self { claim_amount }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<RemoveLiquidityReceipt> {
        require!(self.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            self.claim_amount,
            ErrorCode::InsufficientMarketClaimCoverage
        );
        require_gte!(
            market_side.reserve_ledger.cash_reserve,
            self.claim_amount,
            ErrorCode::InsufficientMarketClaimCoverage
        );

        let next_claim_supply = market_side
            .claim_token_ledger
            .protected_claim_token_supply
            .checked_sub(self.claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let next_required_buffer = required_buffer_for_claims(
            next_claim_supply,
            market_side.buffer_ledger.buffer_ratio_bps,
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
        market_side.claim_token_ledger.protected_claim_token_supply = next_claim_supply;
        market_side.buffer_ledger.required_buffer = next_required_buffer;

        Ok(RemoveLiquidityReceipt {
            claim_amount: self.claim_amount,
            protected_claim_token_supply: market_side
                .claim_token_ledger
                .protected_claim_token_supply,
            required_buffer: market_side.buffer_ledger.required_buffer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::{BufferLedger, MarketSide},
        transitions::fee::RecordFeeCredit,
    };
    use proptest::prelude::*;

    fn market_side(buffer_ratio_bps: u16) -> MarketSide {
        MarketSide {
            buffer_ledger: BufferLedger {
                buffer_ratio_bps,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    #[test]
    fn add_liquidity_mints_claim_minus_buffer() {
        let mut market_side = market_side(2_000);

        let receipt = AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(receipt.claim_amount, 800_000);
        assert_eq!(receipt.buffer_amount, 200_000);
        assert_eq!(market_side.reserve_ledger.live_reserve, 1_000_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 1_000_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            800_000
        );
        assert_eq!(market_side.buffer_ledger.buffer_share_supply, 200_000);
        assert_eq!(market_side.buffer_ledger.required_buffer, 200_000);
        assert_eq!(market_side.claim_floor().unwrap(), 1_000_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn remove_liquidity_redeems_fixed_one_to_one_principal() {
        let mut market_side = market_side(2_000);
        AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();
        RecordFeeCredit::new(10_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        RemoveLiquidity::new(100_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.reserve_ledger.live_reserve, 900_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 900_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            700_000
        );
        assert_eq!(market_side.buffer_ledger.required_buffer, 175_000);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 10_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 10_000);
        market_side.assert_claim_coverage().unwrap();
    }

    proptest! {
        #[test]
        fn add_liquidity_preserves_principal_and_claim_floor(
            reserve_credit in 10_000_u64..1_000_000_000_u64,
            buffer_ratio_bps in 1_u16..BPS_DENOMINATOR,
        ) {
            let mut market_side = market_side(buffer_ratio_bps);

            let receipt = AddLiquidity::new(reserve_credit)
                .apply(&mut market_side)
                .unwrap();

            prop_assert_eq!(
                receipt.claim_amount
                    .checked_add(receipt.buffer_amount)
                    .unwrap(),
                reserve_credit
            );
            prop_assert_eq!(receipt.reserve_credit, reserve_credit);
            prop_assert_eq!(market_side.reserve_ledger.live_reserve, reserve_credit);
            prop_assert_eq!(market_side.reserve_ledger.cash_reserve, reserve_credit);
            prop_assert_eq!(
                market_side.claim_token_ledger.protected_claim_token_supply,
                receipt.claim_amount
            );
            prop_assert_eq!(
                market_side.buffer_ledger.buffer_share_supply,
                receipt.buffer_amount
            );
            prop_assert_eq!(market_side.buffer_ledger.required_buffer, receipt.required_buffer);
            prop_assert!(market_side.buffer_ledger.buffer_share_supply >= receipt.required_buffer);
            market_side.assert_claim_coverage().unwrap();
        }

        #[test]
        fn remove_liquidity_preserves_floor_and_buffer_shares(
            reserve_credit in 10_000_u64..1_000_000_000_u64,
            buffer_ratio_bps in 1_u16..BPS_DENOMINATOR,
            redeem_bps in 1_u16..=BPS_DENOMINATOR,
        ) {
            let mut market_side = market_side(buffer_ratio_bps);
            let add_receipt = AddLiquidity::new(reserve_credit)
                .apply(&mut market_side)
                .unwrap();
            let claim_supply_before = market_side
                .claim_token_ledger
                .protected_claim_token_supply;
            let buffer_share_supply_before = market_side.buffer_ledger.buffer_share_supply;
            let live_reserve_before = market_side.reserve_ledger.live_reserve;
            let cash_reserve_before = market_side.reserve_ledger.cash_reserve;
            let redeem_amount = ((add_receipt.claim_amount as u128)
                .checked_mul(redeem_bps as u128)
                .unwrap()
                .checked_div(BPS_DENOMINATOR as u128)
                .unwrap())
                .max(1) as u64;
            let redeem_amount = redeem_amount.min(add_receipt.claim_amount);

            let remove_receipt = RemoveLiquidity::new(redeem_amount)
                .apply(&mut market_side)
                .unwrap();

            prop_assert_eq!(remove_receipt.claim_amount, redeem_amount);
            prop_assert_eq!(
                market_side.claim_token_ledger.protected_claim_token_supply,
                claim_supply_before.checked_sub(redeem_amount).unwrap()
            );
            prop_assert_eq!(
                market_side.reserve_ledger.live_reserve,
                live_reserve_before.checked_sub(redeem_amount).unwrap()
            );
            prop_assert_eq!(
                market_side.reserve_ledger.cash_reserve,
                cash_reserve_before.checked_sub(redeem_amount).unwrap()
            );
            prop_assert_eq!(
                market_side.buffer_ledger.buffer_share_supply,
                buffer_share_supply_before
            );
            prop_assert_eq!(
                market_side.buffer_ledger.required_buffer,
                remove_receipt.required_buffer
            );
            prop_assert!(market_side.buffer_ledger.buffer_share_supply >= remove_receipt.required_buffer);
            market_side.assert_claim_coverage().unwrap();
        }
    }
}
