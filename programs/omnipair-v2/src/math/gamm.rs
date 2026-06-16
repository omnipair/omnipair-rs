use anchor_lang::prelude::*;

use crate::{
    constants::{MIN_LIQUIDITY, NAD},
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

/// Constructs virtual reserves at pessimistic price = min(P_directional_ema, P_symmetric_ema) from spot reserves
/// - x_virt  = sqrt(k / P_pessimistic) [`collateral_ema_reserve`]
/// - y_virt = sqrt(k * P_pessimistic) [`debt_ema_reserve`]
pub(crate) fn construct_normalized_virtual_reserves_at_pessimistic_price(
    collateral_spot_reserve: u128,
    debt_spot_reserve: u128,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
) -> Result<(u128, u128)> {
    // Minimum liquidity check to prevent sqrt precision loss
    if collateral_spot_reserve < MIN_LIQUIDITY as u128
        || debt_spot_reserve < MIN_LIQUIDITY as u128
    {
        return Ok((0, 0));
    }
    let pessimistic_price =
        collateral_ema_price_nad.min(collateral_directional_ema_price_nad) as u128;
    if pessimistic_price == 0 {
        return Ok((collateral_spot_reserve, debt_spot_reserve));
    }

    let spot_k = collateral_spot_reserve
        .checked_mul(debt_spot_reserve)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    // k * NAD / P_pessimistic
    // Try direct multiplication first; on overflow, split as (R_c * NAD / P) * R_d
    // to keep intermediates within u128 (at a small precision cost).
    let x_virt_squared = match spot_k.checked_mul(NAD as u128) {
        Some(value) => value
            .checked_div(pessimistic_price)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = collateral_spot_reserve
                .checked_mul(NAD as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?
                .checked_div(pessimistic_price)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(debt_spot_reserve)
                .ok_or(ErrorCode::MarketMathOverflow)?
        }
    };
    // sqrt(k * NAD / P_pessimistic)
    let collateral_ema_reserve = x_virt_squared
        .sqrt()
        .ok_or(ErrorCode::MarketMathOverflow)?;

    // k * P_pessimistic / NAD
    // Try direct multiplication first; on overflow, split as (R_d * P / NAD) * R_c.
    let y_virt_squared = match spot_k.checked_mul(pessimistic_price) {
        Some(value) => value
            .checked_div(NAD as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = debt_spot_reserve
                .checked_mul(pessimistic_price)
                .ok_or(ErrorCode::MarketMathOverflow)?
                .checked_div(NAD as u128)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(collateral_spot_reserve)
                .ok_or(ErrorCode::MarketMathOverflow)?
        }
    };
    // sqrt(k * P_pessimistic / NAD)
    let debt_ema_reserve = y_virt_squared
        .sqrt()
        .ok_or(ErrorCode::MarketMathOverflow)?;

    Ok((collateral_ema_reserve, debt_ema_reserve))
}

/// Calculate amount out given amount in.
/// ```text
/// Δy = (Δx * y) / (x + Δx)
/// amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
/// ```
pub(crate) fn calculate_normalized_amount_out(
    reserve_in: u128,
    reserve_out: u128,
    amount_in: u128,
) -> Result<u128> {
    let denominator = reserve_in
        .checked_add(amount_in)
        .ok_or(ErrorCode::DenominatorOverflow)?;
    let amount_out = amount_in
        .checked_mul(reserve_out)
        .ok_or(ErrorCode::OutputAmountOverflow)?
        .checked_div(denominator)
        .ok_or(ErrorCode::OutputAmountOverflow)?;
    Ok(amount_out)
}

/// Calculate amount in required to obtain a given amount out.
/// ```text
/// Δx = (Δy * x) / (y - Δy)
/// amount_in = (amount_out * reserve_in) / (reserve_out - amount_out)
/// ```
pub(crate) fn calculate_normalized_amount_in(
    reserve_in: u128,
    reserve_out: u128,
    amount_out: u128,
) -> Result<u128> {
    let denominator = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::DenominatorOverflow)?;
    let numerator = amount_out
        .checked_mul(reserve_in)
        .ok_or(ErrorCode::OutputAmountOverflow)?;
    let amount_in = ceil_div(numerator, denominator).ok_or(ErrorCode::OutputAmountOverflow)?;
    Ok(amount_in)
}

pub(crate) fn calculate_normalized_amount_in_floor(
    reserve_in: u128,
    reserve_out: u128,
    amount_out: u128,
) -> Result<u128> {
    if amount_out == 0 {
        return Ok(0);
    }
    let denominator = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::DenominatorOverflow)?;
    require!(denominator > 0, ErrorCode::DenominatorOverflow);
    amount_out
        .checked_mul(reserve_in)
        .ok_or(ErrorCode::OutputAmountOverflow)?
        .checked_div(denominator)
        .ok_or(ErrorCode::OutputAmountOverflow.into())
}
