use anchor_lang::prelude::*;
use crate::utils::math::SqrtU128;
use crate::constants::*;
use std::cmp::min;
use crate::errors::ErrorCode;

const NAD_U128: u128 = NAD as u128;
const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;

/// Exact curve solution: given V (NAD-scaled Y) and R1 (raw Y units),
/// solve Y from Y = V * (1 - Y/R1)^2.
/// Let a = V/R1, t = Y/R1. Then
///   t = 2a / (2a + 1 + sqrt(4a + 1))
/// Returns Y as NAD-scaled Y.
#[inline]
fn curve_y_from_v(v: u128, r1: u64) -> u128 {
    if v == 0 || r1 == 0 {
        return 0;
    }

    // a_scaled = a * NAD = (V/R1) * NAD
    let a_scaled = v
        .saturating_mul(NAD_U128)
        / (r1 as u128);

    // sqrt_term_scaled = NAD * sqrt(4a + 1)
    // where 4a + 1 = (4*a_scaled + NAD) / NAD
    // => sqrt_term_scaled = NAD * sqrt(4*a_scaled + NAD) / sqrt(NAD)
    let sqrt_nad = NAD_U128.sqrt().expect("sqrt(NAD) must succeed");
    let four_a_plus_one_num = a_scaled
        .saturating_mul(4)
        .saturating_add(NAD_U128);
    let sqrt_num = four_a_plus_one_num.sqrt().unwrap_or(0);
    let sqrt_term_scaled = NAD_U128
        .saturating_mul(sqrt_num)
        / sqrt_nad;

    // t_scaled = NAD * 2a / (2a + 1 + sqrt(4a+1))
    // with all terms scaled to NAD
    let two_a_scaled = a_scaled.saturating_mul(2);
    let denom = two_a_scaled
        .saturating_add(NAD_U128)        // + 1
        .saturating_add(sqrt_term_scaled) // + sqrt(4a+1)
        .max(1);

    let t_scaled = NAD_U128
        .saturating_mul(two_a_scaled)
        / denom;

    // Y = R1 * t
    (r1 as u128)
        .saturating_mul(t_scaled)
        / NAD_U128
}

/// Maximum borrowable amount of tokenY using an impact-aware CF derived
/// from constant product AMM pricing mechanics, with pessimistic spot/ema cap.
///
/// Inputs:
/// - collateral_amount_scaled: X (NAD-scaled)
/// - collateral_ema_price_scaled: P_ema (NAD-scaled, Y/X)
/// - collateral_spot_price_scaled: P_spot (NAD-scaled, Y/X)
/// - debt_amm_reserve: R1 (raw Y units)
///
/// Returns:
/// - final_borrow_limit (NAD-scaled Y)
/// - max_allowed_cf_bps (pessimistic_cf_bps - LTV_BUFFER_BPS)
/// - liquidation_cf_bps 
pub fn pessimistic_max_debt(
    collateral_amount_scaled: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    debt_amm_reserve: u64,
) -> Result<(u64, u16, u16)> {
    // sanity checks
    if debt_amm_reserve == 0
        || collateral_amount_scaled == 0
        || collateral_ema_price_scaled == 0
        || collateral_spot_price_scaled == 0
    {
        return Ok((0, 0, 0));
    }

    // V = X * P_ema / NAD  (NAD-scaled Y)
    let v = (collateral_amount_scaled as u128)
        .saturating_mul(collateral_ema_price_scaled as u128)
        / NAD_U128;

    // Exact curve solution Y_curve (NAD-scaled)
    let y_curve = curve_y_from_v(v, debt_amm_reserve);

    // CF_curve = Y / V (bps)
    let cf_curve_bps_u128 = if v > 0 {
        y_curve
            .saturating_mul(BPS_DENOMINATOR_U128)
            / v
    } else {
        0
    };
    let cf_curve_bps_u64 = cf_curve_bps_u128 as u64;

    // Pessimistic cap: min(CF, CF * spot/ema) [>= 1 bps, Pessimistic CF <= 8500 bps]
    let liquidation_cf_bps = get_pessimistic_cf_bps(
        cf_curve_bps_u64,
        collateral_spot_price_scaled,
        collateral_ema_price_scaled,
    )?;

    // Max allowed CF BPS = pessimistic CF BPS - LTV_BUFFER_BPS
    let max_allowed_cf_bps = liquidation_cf_bps.saturating_sub(LTV_BUFFER_BPS);

    // Max allowed Y = V * max_allowed_cf_bps / BPS
    let max_allowed_y: u128 = v
        .saturating_mul(max_allowed_cf_bps as u128)
        .checked_div(BPS_DENOMINATOR_U128)
        .unwrap_or(0);

    // Apply LTV buffer: reduce borrow limit by LTV_BUFFER_BPS to create a buffer before liquidation
    // borrow_limit = max_allowed_y * (1 - LTV_BUFFER_BPS / BPS_DENOMINATOR)
    let ltv_buffer_scaled = BPS_DENOMINATOR_U128.saturating_sub(LTV_BUFFER_BPS as u128);
    let final_borrow_limit = max_allowed_y
        .saturating_mul(ltv_buffer_scaled)
        .checked_div(BPS_DENOMINATOR_U128)
        .unwrap_or(0)
        .min(u64::MAX as u128) as u64;

    Ok((final_borrow_limit, max_allowed_cf_bps, liquidation_cf_bps))
}

/// Returns a pessimistic collateral factor in bps:
///   CF_final = min(CF_base, CF_base * spot/ema)
/// Clamped to [100, MAX_COLLATERAL_FACTOR_BPS] bps to avoid zero-division downstream.
/// The dynamic collateral factor is capped at 85% (8500 BPS).
pub fn get_pessimistic_cf_bps(
    base_cf_bps: u64,
    collateral_spot_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u16> {
    require!(collateral_ema_price_nad != 0, ErrorCode::DenominatorOverflow);

    let base = base_cf_bps;
    let shrunk = collateral_spot_price_nad
        .saturating_mul(base)
        .checked_div(collateral_ema_price_nad)
        .ok_or(ErrorCode::DenominatorOverflow)?;

    let cf_bps = min(base, shrunk).max(100); // never less than 1% (100 bps)
    
    // Apply 85% cap to dynamic collateral factor
    let cf_bps_capped = cf_bps.min(MAX_COLLATERAL_FACTOR_BPS as u64);
    Ok(cf_bps_capped as u16)
}
