use anchor_lang::prelude::*;

use crate::{constants::NAD, errors::ErrorCode, state::futarchy_authority::ProtocolAuctionLane};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Fees {
    pub swap_fee_growth_index_nad: u128,
    pub interest_growth_index_nad: u128,
    pub swap_fee_vault_balance: u64,
    pub interest_vault_balance: u64,
    pub swap_fee_liability: u64,
    pub interest_liability: u64,
    pub unallocated_swap_fee_liability: u64,
    pub unallocated_interest_liability: u64,
    pub protocol_fee_liability: u64,
    pub buyback_fee_liability: u64,
    pub manager_swap_fee_liability: u64,
    pub manager_interest_fee_liability: u64,
}

pub fn accrue_fee_liability(
    shares: u64,
    fee_growth_index_nad: u128,
    fee_growth_checkpoint_nad: u128,
) -> Result<u64> {
    if shares == 0 || fee_growth_index_nad <= fee_growth_checkpoint_nad {
        return Ok(0);
    }
    let delta = fee_growth_index_nad
        .checked_sub(fee_growth_checkpoint_nad)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let accrued = (shares as u128)
        .checked_mul(delta)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(accrued).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

impl Fees {
    pub fn total_liability(&self) -> Result<u64> {
        self.swap_fee_liability
            .checked_add(self.interest_liability)
            .and_then(|value| value.checked_add(self.unallocated_swap_fee_liability))
            .and_then(|value| value.checked_add(self.unallocated_interest_liability))
            .and_then(|value| value.checked_add(self.protocol_fee_liability))
            .and_then(|value| value.checked_add(self.buyback_fee_liability))
            .and_then(|value| value.checked_add(self.manager_swap_fee_liability))
            .and_then(|value| value.checked_add(self.manager_interest_fee_liability))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }

    pub fn assert_backed(&self) -> Result<()> {
        let total_vault_balance = self
            .swap_fee_vault_balance
            .checked_add(self.interest_vault_balance)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(
            total_vault_balance,
            self.total_liability()?,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(())
    }

    pub fn protocol_auction_liability(&self, lane: ProtocolAuctionLane) -> u64 {
        match lane {
            ProtocolAuctionLane::Fee => self.protocol_fee_liability,
            ProtocolAuctionLane::Buyback => self.buyback_fee_liability,
        }
    }

    pub fn settle_protocol_auction_liability(
        &mut self,
        lane: ProtocolAuctionLane,
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, ErrorCode::AmountZero);
        match lane {
            ProtocolAuctionLane::Fee => {
                self.protocol_fee_liability = self
                    .protocol_fee_liability
                    .checked_sub(amount)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            ProtocolAuctionLane::Buyback => {
                self.buyback_fee_liability = self
                    .buyback_fee_liability
                    .checked_sub(amount)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/state/fees.rs");
}
