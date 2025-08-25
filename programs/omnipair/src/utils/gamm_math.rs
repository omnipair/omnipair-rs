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

/// Maximum borrowable amount of tokenY using a slippage-aware CF derived
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
/// - pessimistic_cf_bps (u16)
pub fn pessimistic_max_debt(
    collateral_amount_scaled: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    debt_amm_reserve: u64,
) -> Result<(u64, u16)> {
    // sanity checks
    if debt_amm_reserve == 0
        || collateral_amount_scaled == 0
        || collateral_ema_price_scaled == 0
        || collateral_spot_price_scaled == 0
    {
        return Ok((0, 0));
    }

    // V = X * P_ema / NAD  (NAD-scaled Y)
    let v = (collateral_amount_scaled as u128)
        .saturating_mul(collateral_ema_price_scaled as u128)
        / NAD_U128;

    // Exact curve solution Y_curve (NAD-scaled)
    let y_curve = curve_y_from_v(v, debt_amm_reserve);

    // CF_curve = Y / V (bps)
    let mut cf_curve_bps_u128 = if v > 0 {
        y_curve
            .saturating_mul(BPS_DENOMINATOR_U128)
            / v
    } else {
        0
    };
    if cf_curve_bps_u128 > BPS_DENOMINATOR_U128 {
        cf_curve_bps_u128 = BPS_DENOMINATOR_U128;
    }
    let cf_curve_bps_u64 = cf_curve_bps_u128 as u64;

    // Pessimistic cap: min(CF, CF * spot/ema), clamped >= 1 bps
    let pessimistic_cf_bps = get_pessimistic_cf_bps(
        cf_curve_bps_u64,
        collateral_spot_price_scaled,
        collateral_ema_price_scaled,
    )?;

    // Final Y = V * CF_final / BPS
    let max_allowed_y = v
        .saturating_mul(pessimistic_cf_bps as u128)
        / BPS_DENOMINATOR_U128;

    let final_borrow_limit = max_allowed_y
        .min(u64::MAX as u128) as u64;

    Ok((final_borrow_limit, pessimistic_cf_bps))
}

/// Required minimum collateral (tokenX) to borrow a given amount of tokenY,
/// using exact inversion of the curve and the pessimistic spot/ema cap.
///
/// Given Y and R1:
///   t = Y/R1
///   CF_curve = (1 - t)^2
/// Apply pessimistic cap to CF, then
///   V = Y / CF_final
///   X = ceil(V / P_ema)
///
/// Returns:
/// - required_collateral (X, NAD-scaled)
/// - effective_cf_bps (u16)
pub fn pessimistic_min_collateral(
    desired_borrow_y: u64,              // Y (NAD-scaled)
    collateral_ema_price_scaled: u64,   // P_ema (NAD-scaled)
    collateral_spot_price_scaled: u64,  // P_spot (NAD-scaled)
    debt_amm_reserve: u64,              // R1 (raw Y units)
) -> Result<(u64, u16)> {
    if desired_borrow_y == 0
        || collateral_ema_price_scaled == 0
        || collateral_spot_price_scaled == 0
        || debt_amm_reserve == 0
    {
        return Ok((0, 0));
    }

    let y = desired_borrow_y as u128;
    let r1 = debt_amm_reserve as u128;

    // Must have Y < R1 so (1 - Y/R1) > 0
    if y >= r1 {
        return Err(ErrorCode::BorrowExceedsReserve.into());
    }

    // t = Y/R1 (NAD-scaled)
    let t_scaled = y
        .saturating_mul(NAD_U128)
        / r1;

    // CF_curve (NAD-scaled) = (1 - t)^2
    let one_minus_t = NAD_U128.saturating_sub(t_scaled);
    let cf_curve_nad = one_minus_t
        .saturating_mul(one_minus_t)
        / NAD_U128;

    // Convert CF_curve to bps
    let mut cf_curve_bps_u128 = cf_curve_nad
        .saturating_mul(BPS_DENOMINATOR_U128)
        / NAD_U128;
    if cf_curve_bps_u128 > BPS_DENOMINATOR_U128 {
        cf_curve_bps_u128 = BPS_DENOMINATOR_U128;
    }
    let cf_curve_bps_u64 = cf_curve_bps_u128 as u64;

    // Apply pessimistic cap (>= 1 bps)
    let final_cf_bps_u16 = get_pessimistic_cf_bps(
        cf_curve_bps_u64,
        collateral_spot_price_scaled,
        collateral_ema_price_scaled,
    )?;
    let final_cf_bps_u128 = final_cf_bps_u16 as u128;

    // V_final = ceil(Y * BPS / CF_final)
    let v_final = y
        .saturating_mul(BPS_DENOMINATOR_U128)
        .div_ceil(final_cf_bps_u128.max(1));

    // X = ceil(V / P_ema)
    let x = v_final
        .saturating_mul(NAD_U128)
        .div_ceil(collateral_ema_price_scaled as u128);

    Ok((x.min(u64::MAX as u128) as u64, final_cf_bps_u16))
}

/// Returns a pessimistic collateral factor in bps:
///   CF_final = min(CF_base, CF_base * spot/ema)
/// Clamped to [1, 10_000] bps to avoid zero-division downstream.
pub fn get_pessimistic_cf_bps(
    base_cf_bps: u64,
    collateral_spot_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u16> {
    require!(collateral_ema_price_nad != 0, ErrorCode::DenominatorOverflow);

    let base = base_cf_bps.min(BPS_DENOMINATOR as u64);
    let shrunk = collateral_spot_price_nad
        .saturating_mul(base)
        .checked_div(collateral_ema_price_nad)
        .ok_or(ErrorCode::DenominatorOverflow)?;

    let cf_bps = min(base, shrunk).max(1); // never 0 bps
    Ok(cf_bps as u16)
}
