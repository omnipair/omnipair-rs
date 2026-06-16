use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, state::InsuranceReserve};

pub struct DepositInsurance {
    pub market_side_index: u8,
    pub insurance_credit: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InsuranceReceipt {
    pub insurance_credit: u64,
    pub available0: u64,
    pub available1: u64,
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
                insurance_reserve.available0 = insurance_reserve
                    .available0
                    .checked_add(self.insurance_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            1 => {
                insurance_reserve.available1 = insurance_reserve
                    .available1
                    .checked_add(self.insurance_credit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            _ => return err!(ErrorCode::InvalidMarketSide),
        }
        Ok(InsuranceReceipt {
            insurance_credit: self.insurance_credit,
            available0: insurance_reserve.available0,
            available1: insurance_reserve.available1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_insurance_credits_side0() {
        let mut insurance_reserve = InsuranceReserve {
            available0: 10,
            available1: 20,
            ..InsuranceReserve::default()
        };

        let receipt = DepositInsurance::new(0, 15)
            .apply(&mut insurance_reserve)
            .unwrap();

        assert_eq!(receipt.insurance_credit, 15);
        assert_eq!(receipt.available0, 25);
        assert_eq!(receipt.available1, 20);
        assert_eq!(insurance_reserve.available0, 25);
    }

    #[test]
    fn deposit_insurance_credits_side1() {
        let mut insurance_reserve = InsuranceReserve {
            available0: 10,
            available1: 20,
            ..InsuranceReserve::default()
        };

        let receipt = DepositInsurance::new(1, 15)
            .apply(&mut insurance_reserve)
            .unwrap();

        assert_eq!(receipt.insurance_credit, 15);
        assert_eq!(receipt.available0, 10);
        assert_eq!(receipt.available1, 35);
        assert_eq!(insurance_reserve.available1, 35);
    }
}
