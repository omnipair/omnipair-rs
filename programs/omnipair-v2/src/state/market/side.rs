use anchor_lang::prelude::*;

use super::{DailyLimits, Fees, ReserveShares, Reserves};
use crate::{
    constants::{BPS_DENOMINATOR, NAD},
    errors::ErrorCode,
    state::{ProtocolAuctionSplit, YieldAccount},
};

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
    fn from_side(market_side: &MarketSide) -> Self {
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketAsset {
    Base,
    Quote,
}

impl MarketAsset {
    pub fn code(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Quote => 1,
        }
    }

    pub fn try_from_code(code: u8) -> Result<Self> {
        match code {
            0 => Ok(Self::Base),
            1 => Ok(Self::Quote),
            _ => err!(ErrorCode::InvalidArgument),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Base => Self::Quote,
            Self::Quote => Self::Base,
        }
    }

    pub fn is_base(self) -> bool {
        matches!(self, Self::Base)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSide {
    pub asset_mint: Pubkey,
    pub asset_decimals: u8,
    pub hlp_mint: Pubkey,
    pub reserve_vault: Pubkey,
    pub collateral_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub interest_vault: Pubkey,
    pub reserves: Reserves,
    pub shares: ReserveShares,
    pub fees: Fees,
    pub daily_limits: DailyLimits,
}

impl MarketSide {
    pub fn assert_share_backing(&self) -> Result<()> {
        if self.shares.ylp_supply == 0 {
            require_eq!(self.reserves.live_reserve, 0, ErrorCode::BrokenInvariant);
        }
        require_gte!(
            self.reserves.live_reserve,
            self.reserves.reserved_liability,
            ErrorCode::InsufficientLiquidity
        );
        Ok(())
    }

    pub fn ylp_exchange_rate_nad(&self) -> Result<u128> {
        if self.shares.ylp_supply == 0 {
            return Ok(0);
        }
        (self.reserves.live_reserve as u128)
            .checked_mul(crate::constants::NAD as u128)
            .and_then(|value| value.checked_div(self.shares.ylp_supply as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn credit_reserve(&mut self, amount: u64, credit_cash: bool) -> Result<()> {
        self.reserves.live_reserve = self
            .reserves
            .live_reserve
            .checked_add(amount)
            .ok_or(ErrorCode::ReserveOverflow)?;
        if credit_cash {
            self.reserves.cash_reserve = self
                .reserves
                .cash_reserve
                .checked_add(amount)
                .ok_or(ErrorCode::ReserveOverflow)?;
        }
        Ok(())
    }

    pub fn debit_reserve(&mut self, amount: u64, debit_cash: bool) -> Result<()> {
        self.reserves.live_reserve = self
            .reserves
            .live_reserve
            .checked_sub(amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        if debit_cash {
            self.reserves.cash_reserve = self
                .reserves
                .cash_reserve
                .checked_sub(amount)
                .ok_or(ErrorCode::CashReserveUnderflow)?;
        }
        Ok(())
    }

    pub fn record_swap_fee_credit(
        &mut self,
        fee_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Result<FeesReceipt> {
        if fee_credit == 0 {
            return Ok(FeesReceipt::from_side(self));
        }
        let (manager_fee, protocol_fee, lp_fee) =
            split_revenue(fee_credit, manager_fee_bps, protocol_fee_bps)?;
        let (fee_auction_amount, buyback_auction_amount) =
            split_protocol_auction_fee(protocol_fee, &protocol_auction_split)?;
        self.fees.swap_fee_vault_balance = self
            .fees
            .swap_fee_vault_balance
            .checked_add(fee_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.manager_swap_fee_liability = self
            .fees
            .manager_swap_fee_liability
            .checked_add(manager_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.protocol_fee_liability = self
            .fees
            .protocol_fee_liability
            .checked_add(fee_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.buyback_fee_liability = self
            .fees
            .buyback_fee_liability
            .checked_add(buyback_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.unallocated_swap_fee_liability = self
            .fees
            .unallocated_swap_fee_liability
            .checked_add(lp_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.carry_forward_swap_fees()?;
        self.fees.assert_backed()?;
        Ok(FeesReceipt::from_side(self))
    }

    pub fn record_interest_credit(
        &mut self,
        interest_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Result<FeesReceipt> {
        if interest_credit == 0 {
            return Ok(FeesReceipt::from_side(self));
        }
        let (manager_fee, protocol_fee, lp_interest) =
            split_revenue(interest_credit, manager_fee_bps, protocol_fee_bps)?;
        let (fee_auction_amount, buyback_auction_amount) =
            split_protocol_auction_fee(protocol_fee, &protocol_auction_split)?;
        self.fees.interest_vault_balance = self
            .fees
            .interest_vault_balance
            .checked_add(interest_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.manager_interest_fee_liability = self
            .fees
            .manager_interest_fee_liability
            .checked_add(manager_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.protocol_fee_liability = self
            .fees
            .protocol_fee_liability
            .checked_add(fee_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.buyback_fee_liability = self
            .fees
            .buyback_fee_liability
            .checked_add(buyback_auction_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.unallocated_interest_liability = self
            .fees
            .unallocated_interest_liability
            .checked_add(lp_interest)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.carry_forward_interest()?;
        self.fees.assert_backed()?;
        Ok(FeesReceipt::from_side(self))
    }

    pub fn carry_forward_swap_fees(&mut self) -> Result<()> {
        let supply = self.shares.ylp_supply;
        if supply == 0 || self.fees.unallocated_swap_fee_liability == 0 {
            return Ok(());
        }
        let growth_delta = growth_delta_nad(self.fees.unallocated_swap_fee_liability, supply)?;
        let allocated = allocated_from_growth(growth_delta, supply)?;
        self.fees.swap_fee_growth_index_nad = self
            .fees
            .swap_fee_growth_index_nad
            .checked_add(growth_delta)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.swap_fee_liability = self
            .fees
            .swap_fee_liability
            .checked_add(allocated)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.unallocated_swap_fee_liability = self
            .fees
            .unallocated_swap_fee_liability
            .checked_sub(allocated)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn carry_forward_interest(&mut self) -> Result<()> {
        let supply = self.shares.ylp_supply;
        if supply == 0 || self.fees.unallocated_interest_liability == 0 {
            return Ok(());
        }
        let growth_delta = growth_delta_nad(self.fees.unallocated_interest_liability, supply)?;
        let allocated = allocated_from_growth(growth_delta, supply)?;
        self.fees.interest_growth_index_nad = self
            .fees
            .interest_growth_index_nad
            .checked_add(growth_delta)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.interest_liability = self
            .fees
            .interest_liability
            .checked_add(allocated)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.unallocated_interest_liability = self
            .fees
            .unallocated_interest_liability
            .checked_sub(allocated)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        Ok(())
    }

    pub fn prepare_yield_claim(
        &mut self,
        yield_account: &mut YieldAccount,
        vault_balance: u64,
        holder_balance: u64,
    ) -> Result<YieldClaimReceipt> {
        self.carry_forward_swap_fees()?;
        self.carry_forward_interest()?;
        yield_account.accrue(
            holder_balance,
            self.fees.swap_fee_growth_index_nad,
            self.fees.interest_growth_index_nad,
        )?;
        let claim_amount = yield_account.claimable_amount()?;
        require!(claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(vault_balance, claim_amount, ErrorCode::UnbackedFeeLiability);
        Ok(YieldClaimReceipt {
            claim_amount,
            swap_fee_amount: yield_account.accrued_swap_fee_amount,
            interest_amount: yield_account.accrued_interest_amount,
            remaining_swap_fee_liability: self.fees.swap_fee_liability,
            remaining_interest_liability: self.fees.interest_liability,
        })
    }

    pub fn settle_yield_claim(
        &mut self,
        yield_account: &mut YieldAccount,
        claim_amount: u64,
        swap_fee_amount: u64,
        interest_amount: u64,
        swap_fee_vault_balance: u64,
        interest_vault_balance: u64,
    ) -> Result<YieldClaimReceipt> {
        self.fees.swap_fee_liability = self
            .fees
            .swap_fee_liability
            .checked_sub(swap_fee_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.interest_liability = self
            .fees
            .interest_liability
            .checked_sub(interest_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.fees.swap_fee_vault_balance = swap_fee_vault_balance;
        self.fees.interest_vault_balance = interest_vault_balance;
        yield_account.clear_claimed();
        self.fees.assert_backed()?;
        Ok(YieldClaimReceipt {
            claim_amount,
            swap_fee_amount,
            interest_amount,
            remaining_swap_fee_liability: self.fees.swap_fee_liability,
            remaining_interest_liability: self.fees.interest_liability,
        })
    }
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
mod fee_tests {
    include!("../../tests/transitions/fee.rs");
}
