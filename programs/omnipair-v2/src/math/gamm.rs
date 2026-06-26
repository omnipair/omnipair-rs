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
        collateral_side.reserves.live_reserve as u128,
        collateral_side.asset_decimals,
    )?;
    let debt_reserve = normalize_to_nad(
        debt_side.reserves.live_reserve as u128,
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

pub(crate) fn market_k_nad(base_side: &MarketSide, quote_side: &MarketSide) -> Result<u128> {
    normalize_to_nad(
        base_side.reserves.live_reserve as u128,
        base_side.asset_decimals,
    )?
    .checked_mul(normalize_to_nad(
        quote_side.reserves.live_reserve as u128,
        quote_side.asset_decimals,
    )?)
    .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn market_liquidity_nad(
    base_side: &MarketSide,
    quote_side: &MarketSide,
) -> Result<u128> {
    market_k_nad(base_side, quote_side)?
        .sqrt()
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

/// Constructs virtual reserves at pessimistic price = min(P_directional_ema, P_symmetric_ema).
/// - x_virt = sqrt(k * NAD / P_pessimistic)
/// - y_virt = sqrt(k * P_pessimistic / NAD)
pub(crate) fn construct_normalized_virtual_reserves_at_pessimistic_price(
    x_spot: u128,
    y_spot: u128,
    x_price_nad: u64,
    x_directional_price_nad: u64,
) -> Result<(u128, u128)> {
    // Minimum liquidity check to prevent sqrt precision loss
    if x_spot < MIN_LIQUIDITY as u128 || y_spot < MIN_LIQUIDITY as u128 {
        return Ok((0, 0));
    }
    let pessimistic_price_nad = x_price_nad.min(x_directional_price_nad) as u128;
    if pessimistic_price_nad == 0 {
        return Ok((x_spot, y_spot));
    }

    let k = x_spot
        .checked_mul(y_spot)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    // k * NAD / P_pessimistic
    // Try direct multiplication first; on overflow, split as (x * NAD / P) * y
    // to keep intermediates within u128 (at a small precision cost).
    let x_virt_squared = match k.checked_mul(NAD as u128) {
        Some(value) => value
            .checked_div(pessimistic_price_nad)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = x_spot
                .checked_mul(NAD as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?
                .checked_div(pessimistic_price_nad)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(y_spot)
                .ok_or(ErrorCode::MarketMathOverflow)?
        }
    };
    // sqrt(k * NAD / P_pessimistic)
    let x_virt = x_virt_squared.sqrt().ok_or(ErrorCode::MarketMathOverflow)?;

    // k * P_pessimistic / NAD
    // Try direct multiplication first; on overflow, split as (y * P / NAD) * x.
    let y_virt_squared = match k.checked_mul(pessimistic_price_nad) {
        Some(value) => value
            .checked_div(NAD as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = y_spot
                .checked_mul(pessimistic_price_nad)
                .ok_or(ErrorCode::MarketMathOverflow)?
                .checked_div(NAD as u128)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(x_spot)
                .ok_or(ErrorCode::MarketMathOverflow)?
        }
    };
    // sqrt(k * P_pessimistic / NAD)
    let y_virt = y_virt_squared.sqrt().ok_or(ErrorCode::MarketMathOverflow)?;

    Ok((x_virt, y_virt))
}

/// Calculate dy for adding dx to a constant-product coordinate.
/// ```text
/// Δy = (Δx * y) / (x + Δx)
/// ```
pub(crate) fn calculate_normalized_amount_out(x: u128, y: u128, dx: u128) -> Result<u128> {
    let denominator = x.checked_add(dx).ok_or(ErrorCode::DenominatorOverflow)?;
    let dy = dx
        .checked_mul(y)
        .ok_or(ErrorCode::OutputAmountOverflow)?
        .checked_div(denominator)
        .ok_or(ErrorCode::OutputAmountOverflow)?;
    Ok(dy)
}

pub(crate) fn calculate_raw_amount_out(x: u64, y: u64, dx: u64) -> Result<u64> {
    let dy = calculate_normalized_amount_out(x as u128, y as u128, dx as u128)?;
    u64::try_from(dy).map_err(|_| ErrorCode::OutputAmountOverflow.into())
}

/// Calculate dx required to remove dy from a constant-product coordinate.
/// ```text
/// Δx = (Δy * x) / (y - Δy)
/// ```
pub(crate) fn calculate_normalized_amount_in(x: u128, y: u128, dy: u128) -> Result<u128> {
    let denominator = y.checked_sub(dy).ok_or(ErrorCode::DenominatorOverflow)?;
    let numerator = dy.checked_mul(x).ok_or(ErrorCode::OutputAmountOverflow)?;
    let dx = ceil_div(numerator, denominator).ok_or(ErrorCode::OutputAmountOverflow)?;
    Ok(dx)
}

pub(crate) fn calculate_normalized_amount_in_floor(x: u128, y: u128, dy: u128) -> Result<u128> {
    if dy == 0 {
        return Ok(0);
    }
    let denominator = y.checked_sub(dy).ok_or(ErrorCode::DenominatorOverflow)?;
    require!(denominator > 0, ErrorCode::DenominatorOverflow);
    dy.checked_mul(x)
        .ok_or(ErrorCode::OutputAmountOverflow)?
        .checked_div(denominator)
        .ok_or(ErrorCode::OutputAmountOverflow.into())
}

#[cfg(test)]
mod tests {
    include!("../tests/math/gamm.rs");
}
