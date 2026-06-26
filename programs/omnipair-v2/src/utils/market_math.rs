use anchor_lang::prelude::*;

use crate::{constants::NAD, errors::ErrorCode};

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

#[cfg(test)]
mod tests {
    include!("../tests/utils/market_math.rs");
}
