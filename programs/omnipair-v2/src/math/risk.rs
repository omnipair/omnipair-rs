use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MS_PER_DAY, NAD, NATURAL_LOG_OF_TWO_NAD, TAYLOR_TERMS},
    errors::ErrorCode,
    shared::math::{slots_to_ms, taylor_exp},
};

use super::{
    fixed_point::{denormalize_from_nad_ceil, denormalize_from_nad_floor, normalize_to_nad},
    gamm::{
        calculate_normalized_amount_in, calculate_normalized_amount_in_floor,
        calculate_normalized_amount_out,
        construct_normalized_virtual_reserves_at_pessimistic_price,
    },
};

pub(crate) fn health_bps(
    recognized_collateral_value_nad: u128,
    effective_debt_nad: u128,
) -> Result<u64> {
    if effective_debt_nad == 0 {
        return Ok(u64::MAX);
    }
    let health = recognized_collateral_value_nad
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(effective_debt_nad))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(health).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn effective_hedged_debt_nad(
    hedged_debt_nad: u128,
    liquidity_ema: u128,
    min_weight_bps: u16,
    gamma_nad: u64,
) -> Result<u128> {
    if hedged_debt_nad == 0 {
        return Ok(0);
    }
    let min_weight = min_weight_bps as u128;
    let variable_weight = (BPS_DENOMINATOR as u128)
        .checked_sub(min_weight)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let gamma_pressure_nad = if liquidity_ema == 0 {
        NAD as u128
    } else {
        (gamma_nad as u128)
            .checked_mul(hedged_debt_nad)
            .and_then(|value| value.checked_div(liquidity_ema))
            .ok_or(ErrorCode::MarketMathOverflow)?
            .min(NAD as u128)
    };
    let weight_bps = min_weight
        .checked_add(
            variable_weight
                .checked_mul(gamma_pressure_nad)
                .and_then(|value| value.checked_div(NAD as u128))
                .ok_or(ErrorCode::MarketMathOverflow)?,
        )
        .ok_or(ErrorCode::MarketMathOverflow)?;
    hedged_debt_nad
        .checked_mul(weight_bps)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn collateral_value_from_pessimistic_reserves_nad(
    collateral_reserve_amount: u64,
    collateral_decimals: u8,
    debt_reserve_amount: u64,
    debt_decimals: u8,
    collateral_amount: u64,
    price_ema_nad: u64,
    directional_price_ema_nad: u64,
) -> Result<u128> {
    if collateral_amount == 0 {
        return Ok(0);
    }
    let collateral_reserve =
        normalize_to_nad(collateral_reserve_amount as u128, collateral_decimals)?;
    let debt_reserve = normalize_to_nad(debt_reserve_amount as u128, debt_decimals)?;
    let collateral_amount = normalize_to_nad(collateral_amount as u128, collateral_decimals)?;
    let (collateral_virtual_reserve, debt_virtual_reserve) =
        construct_normalized_virtual_reserves_at_pessimistic_price(
            collateral_reserve,
            debt_reserve,
            price_ema_nad,
            directional_price_ema_nad,
        )?;
    calculate_normalized_amount_out(
        collateral_virtual_reserve,
        debt_virtual_reserve,
        collateral_amount,
    )
}

pub(crate) fn collateral_amount_for_debt_amount_ceil(
    collateral_reserve_amount: u64,
    collateral_decimals: u8,
    debt_reserve_amount: u64,
    debt_decimals: u8,
    debt_amount: u128,
    price_ema_nad: u64,
    directional_price_ema_nad: u64,
) -> Result<u64> {
    let collateral_reserve =
        normalize_to_nad(collateral_reserve_amount as u128, collateral_decimals)?;
    let debt_reserve = normalize_to_nad(debt_reserve_amount as u128, debt_decimals)?;
    let debt_amount_nad = normalize_to_nad(debt_amount, debt_decimals)?;
    let (collateral_virtual_reserve, debt_virtual_reserve) =
        construct_normalized_virtual_reserves_at_pessimistic_price(
            collateral_reserve,
            debt_reserve,
            price_ema_nad,
            directional_price_ema_nad,
        )?;
    let collateral_amount_nad = calculate_normalized_amount_in(
        collateral_virtual_reserve,
        debt_virtual_reserve,
        debt_amount_nad,
    )?;
    denormalize_from_nad_ceil(collateral_amount_nad, collateral_decimals)
}

pub(crate) fn collateral_amount_for_debt_value_floor(
    collateral_reserve_amount: u64,
    collateral_decimals: u8,
    debt_reserve_amount: u64,
    debt_decimals: u8,
    debt_value_nad: u128,
    price_ema_nad: u64,
    directional_price_ema_nad: u64,
) -> Result<u64> {
    if debt_value_nad == 0 {
        return Ok(0);
    }
    let collateral_reserve =
        normalize_to_nad(collateral_reserve_amount as u128, collateral_decimals)?;
    let debt_reserve = normalize_to_nad(debt_reserve_amount as u128, debt_decimals)?;
    let (collateral_virtual_reserve, debt_virtual_reserve) =
        construct_normalized_virtual_reserves_at_pessimistic_price(
            collateral_reserve,
            debt_reserve,
            price_ema_nad,
            directional_price_ema_nad,
        )?;
    let collateral_amount_nad = calculate_normalized_amount_in_floor(
        collateral_virtual_reserve,
        debt_virtual_reserve,
        debt_value_nad,
    )?;
    denormalize_from_nad_floor(collateral_amount_nad, collateral_decimals)
}

pub(crate) fn ema_u64(
    last_ema: u64,
    input: u64,
    last_slot: u64,
    current_slot: u64,
    half_life_ms: u64,
) -> u64 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    u64::try_from(ema_u128(
        last_ema as u128,
        input as u128,
        last_slot,
        current_slot,
        half_life_ms,
    ))
    .unwrap_or(u64::MAX)
}

pub(crate) fn directional_ema_u64(
    last_ema: u64,
    input: u64,
    last_slot: u64,
    current_slot: u64,
    half_life_ms: u64,
) -> u64 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    input.min(ema_u64(
        last_ema,
        input,
        last_slot,
        current_slot,
        half_life_ms,
    ))
}

pub(crate) fn ema_u128(
    last_ema: u128,
    input: u128,
    last_slot: u64,
    current_slot: u64,
    half_life_ms: u64,
) -> u128 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    let Some(dt) = slots_to_ms(last_slot, current_slot) else {
        return last_ema;
    };
    if dt == 0 || half_life_ms == 0 {
        return last_ema;
    }
    let x = (dt as u128)
        .saturating_mul(NATURAL_LOG_OF_TWO_NAD as u128)
        .checked_div(half_life_ms as u128)
        .unwrap_or(u128::MAX)
        .min(i64::MAX as u128) as i64;
    let alpha = taylor_exp(-x, NAD, TAYLOR_TERMS) as u128;
    input
        .saturating_mul((NAD as u128).saturating_sub(alpha))
        .saturating_add(last_ema.saturating_mul(alpha))
        .checked_div(NAD as u128)
        .unwrap_or(last_ema)
}

pub(crate) fn decayed_daily_bucket(bucket: u64, last_slot: u64, current_slot: u64) -> Result<u64> {
    if bucket == 0 {
        return Ok(0);
    }
    let Some(elapsed_ms) = slots_to_ms(last_slot, current_slot) else {
        return Ok(bucket);
    };
    if elapsed_ms >= MS_PER_DAY {
        return Ok(0);
    }
    let remaining_ms = (MS_PER_DAY - elapsed_ms) as u128;
    let decayed = (bucket as u128)
        .checked_mul(remaining_ms)
        .and_then(|value| value.checked_div(MS_PER_DAY as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(decayed).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub(crate) fn assert_price_divergence(
    spot_price_nad: u64,
    ema_price_nad: u64,
    max_divergence_bps: u16,
) -> Result<()> {
    require!(
        spot_price_nad > 0 && ema_price_nad > 0,
        ErrorCode::InsufficientLiquidity
    );
    let diff = spot_price_nad.abs_diff(ema_price_nad);
    let divergence_bps = (diff as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(ema_price_nad as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require_gte!(
        max_divergence_bps as u128,
        divergence_bps,
        ErrorCode::MarketRiskCircuitBreaker
    );
    Ok(())
}

pub(crate) fn assert_k_drawdown(
    current_k_nad: u128,
    k_ema_nad: u128,
    max_drawdown_bps: u16,
) -> Result<()> {
    if current_k_nad >= k_ema_nad {
        return Ok(());
    }
    require!(k_ema_nad > 0, ErrorCode::InsufficientLiquidity);
    let drawdown_bps = k_ema_nad
        .checked_sub(current_k_nad)
        .and_then(|value| value.checked_mul(BPS_DENOMINATOR as u128))
        .and_then(|value| value.checked_div(k_ema_nad))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require_gte!(
        max_drawdown_bps as u128,
        drawdown_bps,
        ErrorCode::MarketRiskCircuitBreaker
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pessimistic_reserve_value_uses_normalized_curve_output() {
        let value = collateral_value_from_pessimistic_reserves_nad(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();

        assert_eq!(value, 181_818_181);
    }

    #[test]
    fn debt_value_to_collateral_amount_uses_requested_rounding() {
        let ceil_amount = collateral_amount_for_debt_amount_ceil(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();
        let floor_amount = collateral_amount_for_debt_value_floor(
            1_000_000,
            6,
            2_000_000,
            6,
            100_000_000,
            2 * NAD,
            2 * NAD,
        )
        .unwrap();

        assert_eq!(ceil_amount, 52_632);
        assert_eq!(floor_amount, 52_631);
    }
}
