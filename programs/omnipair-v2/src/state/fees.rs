use anchor_lang::prelude::*;

use super::ProtocolAuctionLane;
use crate::errors::ErrorCode;

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
    use super::*;

    #[test]
    fn total_liability_includes_manager_fee_buckets() {
        let mut fees = Fees {
            swap_fee_vault_balance: 700,
            interest_vault_balance: 300,
            manager_swap_fee_liability: 400,
            manager_interest_fee_liability: 100,
            protocol_fee_liability: 250,
            buyback_fee_liability: 50,
            ..Fees::default()
        };

        assert_eq!(fees.total_liability().unwrap(), 800);
        fees.manager_swap_fee_liability = 0;
        fees.manager_interest_fee_liability = 0;
        assert_eq!(fees.total_liability().unwrap(), 300);
    }

    #[test]
    fn auction_liabilities_settle_by_lane() {
        let mut fees = Fees {
            swap_fee_vault_balance: 700,
            protocol_fee_liability: 500,
            buyback_fee_liability: 200,
            ..Fees::default()
        };

        fees.settle_protocol_auction_liability(ProtocolAuctionLane::Fee, 125)
            .unwrap();
        fees.settle_protocol_auction_liability(ProtocolAuctionLane::Buyback, 50)
            .unwrap();

        assert_eq!(fees.protocol_fee_liability, 375);
        assert_eq!(fees.buyback_fee_liability, 150);
        fees.assert_backed().unwrap();
    }
}
