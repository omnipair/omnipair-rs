use crate::utils::math::SqrtU128;
use crate::constants::*;

/// Calculates the maximum borrowable amount of tokenY using a slippage-aware
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
/// - `collateral_spot_price_scaled`: spot price (tokenX/tokenY), scaled by 1e9
/// - `total_debt`: total debt of tokenY in the AMM (borrowed token)
/// - `debt_reserve`: reserve of tokenY in the AMM (borrowed token)
///
/// Returns:
/// - max_borrowable: u64 — the amount of tokenY borrowable
/// - effective_cf_bps: u16 — resulting CF in BPS (e.g., 8500 = 85%)
pub fn max_borrowable_with_safety(
    collateral_amount_scaled: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    total_debt: u64,
    debt_reserve: u64
) -> (u64, u16) {
    const NAD_U128: u128 = NAD as u128;
    const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;
    let sqrt_nad: u128 = NAD_U128.sqrt().expect("NAD sqrt must succeed");

    // sanity checks
    if debt_reserve == 0 || collateral_amount_scaled == 0 || collateral_ema_price_scaled == 0 {
        return (0, 0);
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
    let approx = (v * (NAD_U128 - sqrt_adjusted_nad)) / NAD_U128;

    // Newton-Raphson refinement
    let y = approx;

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

    // Calculate safety coefficient
    let safety_coeff_bps = calculate_safety_coeff(
        compute_volatility_bps(collateral_ema_price_scaled, collateral_spot_price_scaled),
        compute_utilization_bps(total_debt, debt_reserve),
        compute_v_over_r1_bps(collateral_amount_scaled, collateral_ema_price_scaled, debt_reserve)
    );

    // Apply safety coefficient
    let final_borrowable = (refined_y * safety_coeff_bps as u128 / BPS_DENOMINATOR_U128) as u64;

    // Compute effective CF in BPS
    let effective_cf_bps = if v > 0 {
        ((final_borrowable as u128) * BPS_DENOMINATOR_U128 / v) as u16
    } else {
        0
    };

    (final_borrowable, effective_cf_bps)
}


/// Calculates a safety coefficient (in BPS) based on protocol risk factors.
/// Inputs:
/// - `volatility_bps`: Ema/spot divergence: |EMA - spot| / EMA, scaled to BPS
/// - `utilization_bps`: total debt / reserves, scaled to BPS
/// - `v_over_r1_bps`: value of user's collateral / reserves, scaled to BPS
///
/// Returns:
/// - safety_coeff_bps: 10000 means 100% (no discount), 9500 means 95% safe
pub fn calculate_safety_coeff(
    volatility_bps: u64,
    utilization_bps: u64,
    v_over_r1_bps: u64,
) -> u64 {
    let base = 10_000u64;

    // Weights (adjustable based on model tuning)
    let w_vol = 5; // 0.5x
    let w_util = 2; // 0.2x
    let w_liquidity = 3; // 0.3x

    let penalty = (volatility_bps * w_vol / 10)
        .saturating_add(utilization_bps * w_util / 10)
        .saturating_add(v_over_r1_bps * w_liquidity / 10);

    base.saturating_sub(penalty).clamp(9000, 10_000) // never below 90%
}

pub fn compute_v_over_r1_bps(
    collateral_amount: u64,
    collateral_ema_price_scaled: u64,
    debt_reserve: u64,
) -> u64 {
    (collateral_amount as u128)
    .saturating_mul(collateral_ema_price_scaled as u128)
    .saturating_mul(BPS_DENOMINATOR as u128)
    .div_ceil(debt_reserve as u128 * NAD as u128) as u64
}

pub fn compute_utilization_bps(
    total_debt: u64,
    debt_reserve: u64,
) -> u64 {
    (total_debt as u128)
    .saturating_mul(BPS_DENOMINATOR as u128)
    .div_ceil(debt_reserve as u128 * NAD as u128) as u64
}

pub fn compute_volatility_bps(
    ema_price_scaled: u64,
    spot_price_scaled: u64,
) -> u64 {
    // |EMA - spot| / EMA * 10_000
    ema_price_scaled.abs_diff(spot_price_scaled).saturating_mul(BPS_DENOMINATOR).div_ceil(ema_price_scaled)
}