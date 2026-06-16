use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, state::InsuranceReserve};

pub struct DepositInsurance {
    pub market_side_index: u8,
    pub insurance_credit: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InsuranceReceipt {
    pub insurance_credit: u64,
    pub base_available: u64,
    pub quote_available: u64,
}

impl DepositInsurance {
    pub fn new(market_side_index: u8, insurance_credit: u64) -> Self {
        Self {
            market_side_index,
            insurance_credit,
        }
    }

    pub fn apply(self, insurance_reserve: &mut InsuranceReserve) -> Result<InsuranceReceipt> {
        require!(self.insurance_credit > 0, ErrorCode::AmountZero);
        match self.market_side_index {
            0 => {
                insurance_reserve.base_available = insurance_reserve
                    .base_available
                    .checked_add(self.insurance_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            1 => {
                insurance_reserve.quote_available = insurance_reserve
                    .quote_available
                    .checked_add(self.insurance_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            _ => return err!(ErrorCode::InvalidMarketSide),
        }
        Ok(InsuranceReceipt {
            insurance_credit: self.insurance_credit,
            base_available: insurance_reserve.base_available,
            quote_available: insurance_reserve.quote_available,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_insurance_credits_base_side() {
        let mut insurance_reserve = InsuranceReserve {
            base_available: 10,
            quote_available: 20,
            ..InsuranceReserve::default()
        };

        let receipt = DepositInsurance::new(0, 15)
            .apply(&mut insurance_reserve)
            .unwrap();

        assert_eq!(receipt.insurance_credit, 15);
        assert_eq!(receipt.base_available, 25);
        assert_eq!(receipt.quote_available, 20);
        assert_eq!(insurance_reserve.base_available, 25);
    }

    #[test]
    fn deposit_insurance_credits_quote_side() {
        let mut insurance_reserve = InsuranceReserve {
            base_available: 10,
            quote_available: 20,
            ..InsuranceReserve::default()
        };

        let receipt = DepositInsurance::new(1, 15)
            .apply(&mut insurance_reserve)
            .unwrap();

        assert_eq!(receipt.insurance_credit, 15);
        assert_eq!(receipt.base_available, 10);
        assert_eq!(receipt.quote_available, 35);
        assert_eq!(insurance_reserve.quote_available, 35);
    }
}
