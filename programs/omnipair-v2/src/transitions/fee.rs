use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, NAD},
    errors::ErrorCode,
    state::{MarketSide, ProtocolAuctionSplit, YieldAccount},
};

pub struct RecordSwapFeeCredit {
    pub fee_credit: u64,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub protocol_auction_split: ProtocolAuctionSplit,
}

pub struct RecordInterestCredit {
    pub interest_credit: u64,
    pub manager_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub protocol_auction_split: ProtocolAuctionSplit,
}

pub struct PrepareYieldClaim {
    pub vault_balance: u64,
    pub holder_balance: u64,
}

pub struct SettleYieldClaim {
    pub claim_amount: u64,
    pub swap_fee_amount: u64,
    pub interest_amount: u64,
    pub swap_fee_vault_balance: u64,
    pub interest_vault_balance: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeesReceipt {
    pub swap_fee_growth_index_nad: u128,
    pub interest_growth_index_nad: u128,
    pub swap_fee_liability: u64,
    pub interest_liability: u64,
    pub unallocated_swap_fee_liability: u64,
    pub unallocated_interest_liability: u64,
    pub manager_swap_fee_liability: u64,
    pub manager_interest_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub buyback_fee_liability: u64,
    pub swap_fee_vault_balance: u64,
    pub interest_vault_balance: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct YieldClaimReceipt {
    pub claim_amount: u64,
    pub swap_fee_amount: u64,
    pub interest_amount: u64,
    pub remaining_swap_fee_liability: u64,
    pub remaining_interest_liability: u64,
}

impl FeesReceipt {
    pub fn from_side(market_side: &MarketSide) -> Self {
        let fees = &market_side.fees;
        Self {
            swap_fee_growth_index_nad: fees.swap_fee_growth_index_nad,
            interest_growth_index_nad: fees.interest_growth_index_nad,
            swap_fee_liability: fees.swap_fee_liability,
            interest_liability: fees.interest_liability,
            unallocated_swap_fee_liability: fees.unallocated_swap_fee_liability,
            unallocated_interest_liability: fees.unallocated_interest_liability,
            manager_swap_fee_liability: fees.manager_swap_fee_liability,
            manager_interest_fee_liability: fees.manager_interest_fee_liability,
            protocol_fee_liability: fees.protocol_fee_liability,
            buyback_fee_liability: fees.buyback_fee_liability,
            swap_fee_vault_balance: fees.swap_fee_vault_balance,
            interest_vault_balance: fees.interest_vault_balance,
        }
    }
}

impl RecordSwapFeeCredit {
    pub fn new(
        fee_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Self {
        Self {
            fee_credit,
            manager_fee_bps,
            protocol_fee_bps,
            protocol_auction_split,
        }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeesReceipt> {
        if self.fee_credit == 0 {
            return Ok(FeesReceipt::from_side(market_side));
        }
        let (manager_fee, protocol_fee, lp_fee) =
            split_revenue(self.fee_credit, self.manager_fee_bps, self.protocol_fee_bps)?;
        let (fee_auction_amount, buyback_auction_amount) =
            split_protocol_auction_fee(protocol_fee, &self.protocol_auction_split)?;
        market_side.fees.swap_fee_vault_balance = market_side
            .fees
            .swap_fee_vault_balance
            .checked_add(self.fee_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.manager_swap_fee_liability = market_side
            .fees
            .manager_swap_fee_liability
            .checked_add(manager_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.protocol_fee_liability = market_side
            .fees
            .protocol_fee_liability
            .checked_add(fee_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.buyback_fee_liability = market_side
            .fees
            .buyback_fee_liability
            .checked_add(buyback_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.unallocated_swap_fee_liability = market_side
            .fees
            .unallocated_swap_fee_liability
            .checked_add(lp_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        carry_forward_swap_fees(market_side)?;
        market_side.fees.assert_backed()?;
        Ok(FeesReceipt::from_side(market_side))
    }
}

impl RecordInterestCredit {
    pub fn new(
        interest_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Self {
        Self {
            interest_credit,
            manager_fee_bps,
            protocol_fee_bps,
            protocol_auction_split,
        }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeesReceipt> {
        if self.interest_credit == 0 {
            return Ok(FeesReceipt::from_side(market_side));
        }
        let (manager_fee, protocol_fee, lp_interest) = split_revenue(
            self.interest_credit,
            self.manager_fee_bps,
            self.protocol_fee_bps,
        )?;
        let (fee_auction_amount, buyback_auction_amount) =
            split_protocol_auction_fee(protocol_fee, &self.protocol_auction_split)?;
        market_side.fees.interest_vault_balance = market_side
            .fees
            .interest_vault_balance
            .checked_add(self.interest_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.manager_interest_fee_liability = market_side
            .fees
            .manager_interest_fee_liability
            .checked_add(manager_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.protocol_fee_liability = market_side
            .fees
            .protocol_fee_liability
            .checked_add(fee_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.buyback_fee_liability = market_side
            .fees
            .buyback_fee_liability
            .checked_add(buyback_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.unallocated_interest_liability = market_side
            .fees
            .unallocated_interest_liability
            .checked_add(lp_interest)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        carry_forward_interest(market_side)?;
        market_side.fees.assert_backed()?;
        Ok(FeesReceipt::from_side(market_side))
    }
}

impl PrepareYieldClaim {
    pub fn new(vault_balance: u64, holder_balance: u64) -> Self {
        Self {
            vault_balance,
            holder_balance,
        }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        yield_account: &mut YieldAccount,
    ) -> Result<YieldClaimReceipt> {
        carry_forward_swap_fees(market_side)?;
        carry_forward_interest(market_side)?;
        yield_account.accrue(
            self.holder_balance,
            market_side.fees.swap_fee_growth_index_nad,
            market_side.fees.interest_growth_index_nad,
        )?;
        let claim_amount = yield_account.claimable_amount()?;
        require!(claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.vault_balance,
            claim_amount,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(YieldClaimReceipt {
            claim_amount,
            swap_fee_amount: yield_account.accrued_swap_fee_amount,
            interest_amount: yield_account.accrued_interest_amount,
            remaining_swap_fee_liability: market_side.fees.swap_fee_liability,
            remaining_interest_liability: market_side.fees.interest_liability,
        })
    }
}

impl SettleYieldClaim {
    pub fn apply(
        self,
        market_side: &mut MarketSide,
        yield_account: &mut YieldAccount,
    ) -> Result<YieldClaimReceipt> {
        market_side.fees.swap_fee_liability = market_side
            .fees
            .swap_fee_liability
            .checked_sub(self.swap_fee_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.interest_liability = market_side
            .fees
            .interest_liability
            .checked_sub(self.interest_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fees.swap_fee_vault_balance = self.swap_fee_vault_balance;
        market_side.fees.interest_vault_balance = self.interest_vault_balance;
        yield_account.clear_claimed();
        market_side.fees.assert_backed()?;
        Ok(YieldClaimReceipt {
            claim_amount: self.claim_amount,
            swap_fee_amount: self.swap_fee_amount,
            interest_amount: self.interest_amount,
            remaining_swap_fee_liability: market_side.fees.swap_fee_liability,
            remaining_interest_liability: market_side.fees.interest_liability,
        })
    }
}

pub fn carry_forward_swap_fees(market_side: &mut MarketSide) -> Result<()> {
    let supply = market_side.shares.ylp_supply;
    if supply == 0 || market_side.fees.unallocated_swap_fee_liability == 0 {
        return Ok(());
    }
    let growth_delta = growth_delta_nad(market_side.fees.unallocated_swap_fee_liability, supply)?;
    let allocated = allocated_from_growth(growth_delta, supply)?;
    market_side.fees.swap_fee_growth_index_nad = market_side
        .fees
        .swap_fee_growth_index_nad
        .checked_add(growth_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fees.swap_fee_liability = market_side
        .fees
        .swap_fee_liability
        .checked_add(allocated)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fees.unallocated_swap_fee_liability = market_side
        .fees
        .unallocated_swap_fee_liability
        .checked_sub(allocated)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

pub fn carry_forward_interest(market_side: &mut MarketSide) -> Result<()> {
    let supply = market_side.shares.ylp_supply;
    if supply == 0 || market_side.fees.unallocated_interest_liability == 0 {
        return Ok(());
    }
    let growth_delta = growth_delta_nad(market_side.fees.unallocated_interest_liability, supply)?;
    let allocated = allocated_from_growth(growth_delta, supply)?;
    market_side.fees.interest_growth_index_nad = market_side
        .fees
        .interest_growth_index_nad
        .checked_add(growth_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fees.interest_liability = market_side
        .fees
        .interest_liability
        .checked_add(allocated)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fees.unallocated_interest_liability = market_side
        .fees
        .unallocated_interest_liability
        .checked_sub(allocated)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

fn split_revenue(amount: u64, manager_bps: u16, protocol_bps: u16) -> Result<(u64, u64, u64)> {
    require_gte!(BPS_DENOMINATOR, manager_bps, ErrorCode::InvalidMarketConfig);
    require_gte!(
        BPS_DENOMINATOR,
        protocol_bps,
        ErrorCode::InvalidMarketConfig
    );
    require_gte!(
        BPS_DENOMINATOR,
        manager_bps
            .checked_add(protocol_bps)
            .ok_or(ErrorCode::InvalidMarketConfig)?,
        ErrorCode::InvalidMarketConfig
    );
    let manager_fee = proportional_bps(amount, manager_bps)?;
    let protocol_fee = proportional_bps(amount, protocol_bps)?;
    let lp_amount = amount
        .checked_sub(manager_fee)
        .and_then(|value| value.checked_sub(protocol_fee))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((manager_fee, protocol_fee, lp_amount))
}

fn split_protocol_auction_fee(
    protocol_fee: u64,
    split: &ProtocolAuctionSplit,
) -> Result<(u64, u64)> {
    require!(split.is_valid(), ErrorCode::InvalidDistribution);
    let buyback_amount = proportional_bps(protocol_fee, split.buyback_auction_bps)?;
    let fee_amount = protocol_fee
        .checked_sub(buyback_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((fee_amount, buyback_amount))
}

fn proportional_bps(amount: u64, bps: u16) -> Result<u64> {
    let value = (amount as u128)
        .checked_mul(bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn growth_delta_nad(amount: u64, supply: u64) -> Result<u128> {
    (amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(supply as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn allocated_from_growth(growth_delta: u128, supply: u64) -> Result<u64> {
    let allocated = growth_delta
        .checked_mul(supply as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(allocated).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

#[cfg(test)]
mod tests {
    include!("../tests/transitions/fee.rs");
}
