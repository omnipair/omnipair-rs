use anchor_lang::prelude::*;
use crate::utils::math::SqrtU128;
use crate::constants::*;
use std::cmp::min;
use crate::errors::ErrorCode;

const NAD_U128: u128 = NAD as u128;
const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;

/// Maximum borrowable amount of tokenY using a slippage-aware
/// collateral factor derived from constant product AMM pricing mechanics.
/// 
/// background: (Collateral token is tokenX, debt token is tokenY)
/// - Exact constraint: Y = V * (1 - Y / R1)^2
/// - We use a symbolic approximation: Y ≈ V * (1 - sqrt(V / R1))
/// - Then we refine with one Newton-Raphson step for accuracy
/// 
/// Inputs:
/// - `collateral_amount_scaled`: amount of tokenX the user deposited, scaled by 1e9
/// - `collateral_ema_price_scaled`: EMA price (tokenX/tokenY), scaled by 1e9
/// - `collateral_spot_price_scaled`: Spot price (tokenX/tokenY), scaled by 1e9
/// - `debt_reserve`: reserve of tokenY in the AMM (borrowed token)
///
/// Returns:
/// - final_borrow_limit: u64 — the amount of tokenY borrowable using a pessimistic CF
/// - pessimistic_cf_bps: u16 — pessimistic CF in BPS (e.g., 8500 = 85%)
pub fn pessimistic_max_debt(
    collateral_amount_scaled: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    debt_reserve: u64,
) -> Result<(u64, u16)> {
    let sqrt_nad: u128 = NAD_U128.sqrt().expect("NAD sqrt must succeed");

    // sanity checks
    if debt_reserve == 0 || collateral_amount_scaled == 0 || collateral_ema_price_scaled == 0 || collateral_spot_price_scaled == 0 {
        return Ok((0, 0));
    }

    // Compute collateral value in tokenY: V = X * P_ema
    let v = (collateral_amount_scaled as u128)
        .saturating_mul(collateral_ema_price_scaled as u128)
        / NAD_U128;

    // Compute sqrt(V / R1) using fixed-point scale
    let v_over_r1 = (v * NAD_U128) / debt_reserve as u128;

    // sqrt(x_scaled) = sqrt(x) * sqrt(1e9) not = sqrt(x) * 1e9
    let sqrt_of_v_ofer_r_and_scale = v_over_r1.sqrt().unwrap_or(0); // result is scaled by 1e9

    // sqrt(x) scaled* = sqrt(x) * 1e9 / sqrt(1e9)
    let sqrt_adjusted_nad = sqrt_of_v_ofer_r_and_scale
    .saturating_mul(NAD_U128)
    .div_ceil(sqrt_nad);

    // Approximate solution: Y ≈ V * (1 - sqrt(V / R1))
    let approx_y = (v * (NAD_U128 - sqrt_adjusted_nad)) / NAD_U128;

    // Newton-Raphson refinement
    let y = approx_y;

    let one_minus_y_r1 = NAD_U128.saturating_sub((y * NAD_U128) / debt_reserve as u128); // (1 - Y / R1)
    let f_y = (v * one_minus_y_r1.saturating_mul(one_minus_y_r1)) / (NAD_U128 * NAD_U128).saturating_sub(y);
    let f_prime = 0u128
        .saturating_sub(2 * v * one_minus_y_r1 / (debt_reserve as u128 * NAD_U128))
        .saturating_sub(1);

    let refined_y = if f_prime != 0 {
        y.saturating_sub(f_y / f_prime)
    } else {
        y
    };

    // Compute applied CF in BPS (CF_curve = Y / V)
    let curve_borrow_limit = refined_y.try_into().unwrap_or(u64::MAX);
    let derived_cf_bps = if v > 0 {
        (curve_borrow_limit as u128) * BPS_DENOMINATOR_U128 / v
    } else {
        0
    };

    // Apply pessimistic CF cap: CF_final = min(CF_curve, spot/ema * CF_curve)
    msg!("derived_cf_bps: {}", derived_cf_bps);
    let pessimistic_cf_bps = get_pessimistic_cf_bps(
        derived_cf_bps as u64, 
        collateral_spot_price_scaled,
        collateral_ema_price_scaled)?;
    msg!("pessimistic_cf_bps: {}", pessimistic_cf_bps);

    // Final Y = V * CF_final (pessimistically capped)
    let max_allowed_y = (v * pessimistic_cf_bps as u128) / BPS_DENOMINATOR_U128;
    msg!("max_allowed_y: {}", max_allowed_y);
    let final_borrow_limit = max_allowed_y.try_into().unwrap_or(u64::MAX);
    msg!("final_borrow_limit: {}", final_borrow_limit);

    Ok((final_borrow_limit, pessimistic_cf_bps))
}

/// Required minimum collateral (tokenX) to borrow a given amount of tokenY,
/// using a symbolic inverse of the slippage-aware borrow formula.
///
/// Assumes:
///     Y ≈ V * (1 - sqrt(V / R1))
/// Approximated inversion:
///     V ≈ Y + sqrt(Y * R1)
///     X = V / P
///
/// Inputs:
/// - `desired_borrow_y`: desired amount of tokenY to borrow (u64)
/// - `collateral_ema_price_scaled`: EMA price (tokenX/tokenY), scaled by 1e9
/// - `collateral_spot_price_scaled`: Spot price (tokenX/tokenY), scaled by 1e9
/// - `debt_reserve`: reserve of tokenY in the AMM (borrowed token)
///
/// Returns:
/// - required_collateral: u64 — estimated tokenX amount needed
/// - effective_cf_bps: u16 — resulting collateral factor in BPS
pub fn pessimistic_min_collateral(
    desired_borrow_y: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    debt_reserve: u64,
) -> Result<(u64, u16)> {
    const NAD_U128: u128 = NAD as u128;
    const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;

    if desired_borrow_y == 0 || collateral_ema_price_scaled == 0 || collateral_spot_price_scaled == 0 || debt_reserve == 0 {
        return Ok((0, 0));
    }

    let y = desired_borrow_y as u128;
    let r1 = debt_reserve as u128;

    // Base symbolic approximation: V ≈ Y + sqrt(Y * R1)
    let sqrt_term = (y.saturating_mul(r1)).sqrt().unwrap_or(0);
    let v_curve = y.saturating_add(sqrt_term);

    // CF_curve = Y / V
    let derived_cf_bps = if v_curve > 0 {
        (y * BPS_DENOMINATOR_U128 / v_curve) as u64
    } else {
        0
    };

    // CF_final = min(CF_curve, (spot/ema) * CF_curve)
    let final_cf_bps = get_pessimistic_cf_bps(
        derived_cf_bps,
        collateral_spot_price_scaled,
        collateral_ema_price_scaled,
    )?;

    // V_final = Y / CF_final
    let v_final = (y * BPS_DENOMINATOR_U128) / final_cf_bps as u128;

    // X = V / P_ema
    let x = v_final
        .saturating_mul(NAD_U128)
        .div_ceil(collateral_ema_price_scaled as u128);

    let required_collateral = x.try_into().unwrap_or(u64::MAX);
    let effective_cf_bps = final_cf_bps as u16;

    Ok((required_collateral, effective_cf_bps))
}


/// Returns a pessimistic collateral factor bps
/// Returns min(CF, SpotPrice / EmaPrice * CF)
pub fn get_pessimistic_cf_bps(base_cf_bps: u64, collateral_spot_price_nad: u64, collateral_ema_price_nad: u64) -> Result<u16> {
    Ok(min(
        base_cf_bps,
        collateral_spot_price_nad
            .checked_mul(base_cf_bps)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(collateral_ema_price_nad)
            .ok_or(ErrorCode::DenominatorOverflow)?
    ) as u16)
}