use anchor_lang::prelude::*;

use crate::{
    constants::NAD,
    errors::ErrorCode,
    shared::math::{ceil_div, SqrtU128},
    state::MarketSide,
};

use super::fixed_point::normalize_to_nad;

pub(crate) fn market_spot_price_nad(
    collateral_side: &MarketSide,
    debt_side: &MarketSide,
) -> Result<u64> {
    let collateral_reserve = normalize_to_nad(
        collateral_side.reserve_ledger.live_reserve as u128,
        collateral_side.asset_decimals,
    )?;
    let debt_reserve = normalize_to_nad(
        debt_side.reserve_ledger.live_reserve as u128,
        debt_side.asset_decimals,
    )?;
    if collateral_reserve == 0 {
        return Ok(0);
    }
    let price = debt_reserve
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(collateral_reserve))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(price).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn market_k_nad(side0: &MarketSide, side1: &MarketSide) -> Result<u128> {
    normalize_to_nad(
        side0.reserve_ledger.live_reserve as u128,
        side0.asset_decimals,
    )?
    .checked_mul(normalize_to_nad(
        side1.reserve_ledger.live_reserve as u128,
        side1.asset_decimals,
    )?)
    .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn market_liquidity_nad(side0: &MarketSide, side1: &MarketSide) -> Result<u128> {
    market_k_nad(side0, side1)?
        .sqrt()
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn virtual_reserves_at_pessimistic_price(
    collateral_reserve: u128,
    debt_reserve: u128,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
) -> Result<(u128, u128)> {
    if collateral_reserve == 0 || debt_reserve == 0 {
        return err!(ErrorCode::InsufficientLiquidity);
    }
    let pessimistic_price =
        collateral_ema_price_nad.min(collateral_directional_ema_price_nad) as u128;
    require!(pessimistic_price > 0, ErrorCode::InvalidMarketConfig);
    let k = collateral_reserve
        .checked_mul(debt_reserve)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let collateral_squared = k
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(pessimistic_price))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let debt_squared = k
        .checked_mul(pessimistic_price)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((
        collateral_squared
            .sqrt()
            .ok_or(ErrorCode::MarketMathOverflow)?,
        debt_squared.sqrt().ok_or(ErrorCode::MarketMathOverflow)?,
    ))
}

pub(crate) fn constant_product_amount_out(
    reserve_in: u128,
    reserve_out: u128,
    amount_in: u128,
) -> Result<u128> {
    if amount_in == 0 {
        return Ok(0);
    }
    let denominator = reserve_in
        .checked_add(amount_in)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    amount_in
        .checked_mul(reserve_out)
        .and_then(|value| value.checked_div(denominator))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn constant_product_amount_in(
    reserve_in: u128,
    reserve_out: u128,
    amount_out: u128,
) -> Result<u128> {
    require_gte!(reserve_out, amount_out, ErrorCode::InsufficientLiquidity);
    let denominator = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require!(denominator > 0, ErrorCode::InsufficientLiquidity);
    ceil_div(
        amount_out
            .checked_mul(reserve_in)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        denominator,
    )
    .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn constant_product_amount_in_floor(
    reserve_in: u128,
    reserve_out: u128,
    amount_out: u128,
) -> Result<u128> {
    if amount_out == 0 {
        return Ok(0);
    }
    require_gte!(reserve_out, amount_out, ErrorCode::InsufficientLiquidity);
    let denominator = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require!(denominator > 0, ErrorCode::InsufficientLiquidity);
    amount_out
        .checked_mul(reserve_in)
        .and_then(|value| value.checked_div(denominator))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}
