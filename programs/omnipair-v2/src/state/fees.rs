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
    pub operator_fee_liability: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MarketFeeClaimKind {
    Operator,
    Protocol,
}

impl MarketFeeClaimKind {
    pub fn event_code(self) -> u8 {
        match self {
            Self::Operator => 0,
            Self::Protocol => 1,
        }
    }
}

impl Fees {
    pub fn total_liability(&self) -> Result<u64> {
        self.swap_fee_liability
            .checked_add(self.interest_liability)
            .and_then(|value| value.checked_add(self.unallocated_swap_fee_liability))
            .and_then(|value| value.checked_add(self.unallocated_interest_liability))
            .and_then(|value| value.checked_add(self.protocol_fee_liability))
            .and_then(|value| value.checked_add(self.buyback_fee_liability))
            .and_then(|value| value.checked_add(self.operator_fee_liability))
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

    pub fn market_fee_liability(&self, claim_kind: MarketFeeClaimKind) -> u64 {
        match claim_kind {
            MarketFeeClaimKind::Operator => self.operator_fee_liability,
            MarketFeeClaimKind::Protocol => self.protocol_fee_liability,
        }
    }

    pub fn claim_market_fee_liability(&mut self, claim_kind: MarketFeeClaimKind) -> Result<u64> {
        let fee_amount = self.market_fee_liability(claim_kind);
        require!(fee_amount > 0, ErrorCode::AmountZero);
        match claim_kind {
            MarketFeeClaimKind::Operator => self.operator_fee_liability = 0,
            MarketFeeClaimKind::Protocol => self.protocol_fee_liability = 0,
        }
        Ok(fee_amount)
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
    fn market_fee_liabilities_settle_operator_and_protocol_buckets() {
        let mut fees = Fees {
            swap_fee_vault_balance: 700,
            operator_fee_liability: 400,
            protocol_fee_liability: 250,
            buyback_fee_liability: 50,
            ..Fees::default()
        };

        let operator_fee = fees
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap();
        let protocol_fee = fees
            .claim_market_fee_liability(MarketFeeClaimKind::Protocol)
            .unwrap();
        let err = fees
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap_err();

        assert_eq!(operator_fee, 400);
        assert_eq!(protocol_fee, 250);
        assert_eq!(fees.operator_fee_liability, 0);
        assert_eq!(fees.protocol_fee_liability, 0);
        assert_eq!(fees.buyback_fee_liability, 50);
        assert_eq!(err, error!(ErrorCode::AmountZero));
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
