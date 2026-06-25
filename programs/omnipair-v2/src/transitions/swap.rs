use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{MarketSide, ProtocolAuctionSplit},
    transitions::fee::{FeesReceipt, RecordSwapFeeCredit},
};

pub struct Swap {
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub protocol_auction_split: ProtocolAuctionSplit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwapReceipt {
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub reserve_in_live_reserve: u64,
    pub reserve_out_live_reserve: u64,
    pub fees: FeesReceipt,
}

impl Swap {
    pub fn new(
        amount_in_after_fee: u64,
        amount_out: u64,
        fee_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Self {
        Self {
            amount_in_after_fee,
            amount_out,
            fee_credit,
            manager_fee_bps,
            protocol_fee_bps,
            protocol_auction_split,
        }
    }

    pub fn apply(
        self,
        market_side_in: &mut MarketSide,
        market_side_out: &mut MarketSide,
    ) -> Result<SwapReceipt> {
        require_gte!(
            market_side_out.reserves.cash_reserve,
            self.amount_out,
            ErrorCode::InsufficientLiquidity
        );

        market_side_in.reserves.live_reserve = market_side_in
            .reserves
            .live_reserve
            .checked_add(self.amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_in.reserves.cash_reserve = market_side_in
            .reserves
            .cash_reserve
            .checked_add(self.amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_out.reserves.live_reserve = market_side_out
            .reserves
            .live_reserve
            .checked_sub(self.amount_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        market_side_out.reserves.cash_reserve = market_side_out
            .reserves
            .cash_reserve
            .checked_sub(self.amount_out)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        let fees = RecordSwapFeeCredit::new(
            self.fee_credit,
            self.manager_fee_bps,
            self.protocol_fee_bps,
            self.protocol_auction_split,
        )
        .apply(market_side_in)?;
        market_side_in.assert_share_backing()?;
        market_side_out.assert_share_backing()?;
        market_side_in.fees.assert_backed()?;

        Ok(SwapReceipt {
            amount_in_after_fee: self.amount_in_after_fee,
            amount_out: self.amount_out,
            fee_credit: self.fee_credit,
            reserve_in_live_reserve: market_side_in.reserves.live_reserve,
            reserve_out_live_reserve: market_side_out.reserves.live_reserve,
            fees,
        })
    }
}
