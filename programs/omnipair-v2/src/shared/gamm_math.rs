use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, shared::math::ceil_div};

/// Constant product curve helper for swap quote math.
pub struct CPCurve;

impl CPCurve {
    pub fn calculate_amount_out(reserve_in: u64, reserve_out: u64, amount_in: u64) -> Result<u64> {
        let denominator = (reserve_in as u128)
            .checked_add(amount_in as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        let amount_out = (amount_in as u128)
            .checked_mul(reserve_out as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;
        Ok(amount_out)
    }

    pub fn calculate_amount_in(reserve_in: u64, reserve_out: u64, amount_out: u64) -> Result<u64> {
        let denominator = (reserve_out as u128)
            .checked_sub(amount_out as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        let numerator = (amount_out as u128)
            .checked_mul(reserve_in as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?;
        let amount_in = ceil_div(numerator, denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;
        Ok(amount_in)
    }
}
