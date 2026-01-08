use anchor_lang::prelude::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use std::cmp::min;

const NAD_U128: u128 = NAD as u128;
const BPS_DENOMINATOR_U128: u128 = BPS_DENOMINATOR as u128;

/// Constant Product Curve (invariant: x * y = k)
///
/// Exposes two functions for computing swaps under the constant-product equality:
///   (x + Δx) * (y − Δy) = x * y
///
/// Provides:
///   - [`CPCurve::calculate_amount_out`]: Given amount_in and reserves, computes amount_out (“how much out for a given in”)
///         Δy = (Δx * y) / (x + Δx)
///   - [`CPCurve::calculate_amount_in`]:  Given desired amount_out and reserves, computes required amount_in (“how much in to get desired out”)
///         Δx = (Δy * x) / (y - Δy)
/// 
/// Assumes no fees and integer division rounding down.
pub struct CPCurve;

impl CPCurve {
    /// Calculate amount out given amount in.
    /// ```text
    /// Δy = (Δx * y) / (x + Δx)
    /// amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
    /// ```
    pub fn calculate_amount_out(reserve_in: u64, reserve_out: u64, amount_in: u64) -> Result<u64> {
        let denominator = (reserve_in as u128)
            .checked_add(amount_in as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        let amount_out = (amount_in as u128)
            .checked_mul(reserve_out as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;
        Ok(amount_out)
    }

    /// Calculate amount in required to obtain a given amount out.
    /// ```text
    /// Δx = (Δy * x) / (y - Δy)
    /// amount_in = (amount_out * reserve_in) / (reserve_out - amount_out)
    /// ```
    pub fn calculate_amount_in(reserve_in: u64, reserve_out: u64, amount_out: u64) -> Result<u64> {
        let denominator = (reserve_out as u128)
            .checked_sub(amount_out as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        let amount_in = (amount_out as u128)
            .checked_mul(reserve_in as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;
        Ok(amount_in)
    }
}

/// Calculates collateral (X) needed to repay a given debt (Y) via AMM swap.
/// Answers: "How much X must be swapped to get `current_total_debt` Y out?"
/// Includes price impact from the constant product curve.
fn calculate_utilized_collateral_with_impact(current_total_debt: u64, collateral_amm_reserve: u64, debt_amm_reserve: u64) -> Result<u64> {
    CPCurve::calculate_amount_in(collateral_amm_reserve, debt_amm_reserve, current_total_debt)
}

/// Calculates the pool's max total debt capacity given utilized + user collateral.
/// Includes price impact from the constant product curve.
fn calculate_max_allowed_total_debt(utilized_collateral: u64, user_collateral_amount: u64, collateral_amm_reserve: u64, debt_amm_reserve: u64) -> Result<u64> {
    let total_collateral_amount = utilized_collateral.checked_add(user_collateral_amount).ok_or(ErrorCode::Overflow)?;
    CPCurve::calculate_amount_out(debt_amm_reserve, collateral_amm_reserve, total_collateral_amount)
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
    collateral_amm_reserve: u64,
    debt_amm_reserve: u64,
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

    // Collateral Value (V) in debt token (Y) = Collateral Amount (X) * Collateral EMA Price (P_ema) / NAD
    // V = X * P_ema / NAD  (NAD-scaled Y)
    let collateral_value = (collateral_amount_scaled as u128)
        .saturating_mul(collateral_ema_price_scaled as u128)
        .checked_div(NAD_U128)
        .ok_or(ErrorCode::Overflow)?;

    // Determine base CF: either fixed CF or dynamic AMM-based CF
    let base_cf_bps: u64 = if let Some(fixed_cf) = fixed_cf_bps {
        // Fixed CF path: use the fixed CF directly as base
        fixed_cf as u64
    } else {
        // Dynamic CF path: calculate impact-aware CF from AMM curve
        if debt_amm_reserve == 0 {
            return Ok((0, 0, 0));
        }

        // 0. Calculate utilized collateral with price impact.
        let utilized_collateral = calculate_utilized_collateral_with_impact(total_debt, collateral_amm_reserve, debt_amm_reserve)?;

        // 1. Calculate max allowed total debt.
        let max_allowed_total_debt = calculate_max_allowed_total_debt(
            utilized_collateral,
            collateral_amount_scaled, 
            collateral_amm_reserve, 
            debt_amm_reserve)?;

        // 2. Calculate user max debt.
        let user_max_debt = max_allowed_total_debt.checked_sub(total_debt).unwrap_or(0);

        // 3. Calculate base CF = user max debt * BPS_DENOMINATOR / Collateral Value (V)
        user_max_debt
        .saturating_mul(BPS_DENOMINATOR_U128 as u64)
        .checked_div(collateral_value as u64).unwrap_or(0) as u64
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
    let final_borrow_limit: u64 = collateral_value
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
