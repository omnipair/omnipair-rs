use anchor_lang::prelude::*;
use crate::utils::math::SqrtU128;
use crate::constants::*;
use crate::errors::ErrorCode;
use std::cmp::min;

const NAD_U128: u128 = NAD as u128;
const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;

// Util functions
fn calculate_utilized_collateral(total_debt: u64, collateral_amm_reserve: u64, debt_amm_reserve: u64) -> Result<u64> {
    let utilized_collateral_denominator = debt_amm_reserve.checked_sub(total_debt).unwrap();
    let utilized_collateral = total_debt.checked_mul(collateral_amm_reserve).ok_or(ErrorCode::Overflow)?.checked_div(utilized_collateral_denominator).ok_or(ErrorCode::Overflow)?;
    Ok(utilized_collateral)
}

fn calculate_max_debt(utilized_collateral: u64, collateral_amm_reserve: u64, debt_amm_reserve: u64) -> Result<u64> {
    let utilized_collateral_plus_user_collateral = utilized_collateral.checked_add(collateral_amm_reserve).unwrap();
    let max_debt_denominator = utilized_collateral_plus_user_collateral.checked_add(collateral_amm_reserve).unwrap();
    let max_debt = utilized_collateral_plus_user_collateral.checked_mul(debt_amm_reserve).unwrap().checked_div(max_debt_denominator).unwrap();
    Ok(max_debt)
}

/// Maximum borrowable amount of tokenY using either a fixed CF or an impact-aware CF
/// derived from constant product AMM pricing mechanics, with pessimistic spot/ema cap.
///
/// Inputs:
/// - collateral_amount_scaled: X (NAD-scaled)
/// - collateral_ema_price_scaled: P_ema (NAD-scaled, Y/X)
/// - collateral_spot_price_scaled: P_spot (NAD-scaled, Y/X)
/// - debt_amm_reserve: R1 (raw Y units) - only used for dynamic CF calculation
/// - fixed_cf_bps: Optional fixed collateral factor. If Some, uses this directly instead of AMM-based CF
///
/// Returns:
/// - final_borrow_limit (NAD-scaled Y)
/// - max_allowed_cf_bps (liquidation_cf_bps * 95%)
/// - liquidation_cf_bps 
pub fn pessimistic_max_debt(
    collateral_amount_scaled: u64,
    collateral_ema_price_scaled: u64,
    collateral_spot_price_scaled: u64,
    debt_amm_reserve: u64,
    collateral_amm_reserve: u64,
    total_debt: u64,
    fixed_cf_bps: Option<u16>,
) -> Result<(u64, u16, u16)> {
    // sanity checks
    if collateral_amount_scaled == 0
        || collateral_ema_price_scaled == 0
        || collateral_spot_price_scaled == 0
    {
        return Ok((0, 0, 0));
    }

    // V = X * P_ema / NAD  (NAD-scaled Y)
    let v = (collateral_amount_scaled as u128)
        .saturating_mul(collateral_ema_price_scaled as u128)
        / NAD_U128;

    // Determine base CF: either fixed CF or dynamic AMM-based CF
    let base_cf_bps: u64 = if let Some(fixed_cf) = fixed_cf_bps {
        // Fixed CF path: use the fixed CF directly as base
        fixed_cf as u64
    } else {
        // Dynamic CF path: calculate impact-aware CF from AMM curve
        if debt_amm_reserve == 0 {
            return Ok((0, 0, 0));
        }

        // 0
        let utilized_collateral = calculate_utilized_collateral(total_debt, collateral_amm_reserve, debt_amm_reserve)?;

        // 1
        let max_debt = calculate_max_debt(utilized_collateral, collateral_amm_reserve, debt_amm_reserve)?;

        // 2
        let user_max_debt = max_debt.checked_sub(total_debt).ok_or(ErrorCode::Overflow)?;

        user_max_debt.checked_mul(BPS_DENOMINATOR_U128 as u64).ok_or(ErrorCode::Overflow)?.checked_div(v as u64).ok_or(ErrorCode::Overflow)? as u64
    };

    // Apply pessimistic spot/EMA divergence cap to prevent EMA lag front-running
    // CF_pessimistic = min(CF_base, CF_base * spot/ema)
    // fixed CF: capped at [100 bps, fixed_cf_bps]
    // dynamic CF: capped at [100, MAX_COLLATERAL_FACTOR_BPS] bps
    let liquidation_cf_bps = if fixed_cf_bps.is_some() {
        // If spot > ema: CF stays at fixed CF (no increase)
        // If spot < ema: CF reduces proportionally to render front-running non-profitable
        require!(collateral_ema_price_scaled != 0, ErrorCode::DenominatorOverflow);
        let base = base_cf_bps as u128;
        let shrunk = (collateral_spot_price_scaled as u128)
            .saturating_mul(base)
            .checked_div(collateral_ema_price_scaled as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        // Apply pessimistic cap: min(fixed_cf_bps, fixed_cf_bps * spot/ema)
        min(base, shrunk).max(100) as u16
    } else {
        // Dynamic CF: apply divergence cap with 85% maximum
        get_pessimistic_cf_bps(
            base_cf_bps,
            collateral_spot_price_scaled,
            collateral_ema_price_scaled,
        )?
    };

    // Max allowed CF BPS = liquidation CF * (1 - LTV_BUFFER_BPS / BPS_DENOMINATOR)
    // This creates a buffer between borrow limit and liquidation threshold
    let max_allowed_cf_bps = ((liquidation_cf_bps as u32)
        .saturating_mul((BPS_DENOMINATOR - LTV_BUFFER_BPS) as u32)
        / BPS_DENOMINATOR as u32) as u16;

    // Final borrow limit = V * max_allowed_cf_bps / BPS
    let final_borrow_limit: u64 = v
        .saturating_mul(max_allowed_cf_bps as u128)
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
