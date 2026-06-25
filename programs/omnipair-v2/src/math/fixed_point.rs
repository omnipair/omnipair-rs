use anchor_lang::prelude::*;

use crate::{constants::NAD_DECIMALS, errors::ErrorCode, shared::math::ceil_div};

pub(crate) fn normalize_to_nad(amount: u128, decimals: u8) -> Result<u128> {
    match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => Ok(amount),
        std::cmp::Ordering::Less => amount
            .checked_mul(
                10_u128
                    .checked_pow((NAD_DECIMALS - decimals) as u32)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
            .ok_or(ErrorCode::MarketMathOverflow.into()),
        std::cmp::Ordering::Greater => Ok(amount
            .checked_div(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
            .ok_or(ErrorCode::MarketMathOverflow)?),
    }
}

pub(crate) fn denormalize_from_nad_ceil(amount_nad: u128, decimals: u8) -> Result<u64> {
    let value = match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => amount_nad,
        std::cmp::Ordering::Less => ceil_div(
            amount_nad,
            10_u128
                .checked_pow((NAD_DECIMALS - decimals) as u32)
                .ok_or(ErrorCode::MarketMathOverflow)?,
        )
        .ok_or(ErrorCode::MarketMathOverflow)?,
        std::cmp::Ordering::Greater => amount_nad
            .checked_mul(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
            .ok_or(ErrorCode::MarketMathOverflow)?,
    };
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn denormalize_from_nad_floor(amount_nad: u128, decimals: u8) -> Result<u64> {
    let value = match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => amount_nad,
        std::cmp::Ordering::Less => amount_nad
            .checked_div(
                10_u128
                    .checked_pow((NAD_DECIMALS - decimals) as u32)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
            .ok_or(ErrorCode::MarketMathOverflow)?,
        std::cmp::Ordering::Greater => amount_nad
            .checked_mul(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
            .ok_or(ErrorCode::MarketMathOverflow)?,
    };
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn observed_or_current_u64(cached_observation: u64, current_observation: u64) -> u64 {
    if cached_observation == 0 {
        current_observation
    } else {
        cached_observation
    }
}

pub(crate) fn observed_or_current_u128(
    cached_observation: u128,
    current_observation: u128,
) -> u128 {
    if cached_observation == 0 {
        current_observation
    } else {
        cached_observation
    }
}
