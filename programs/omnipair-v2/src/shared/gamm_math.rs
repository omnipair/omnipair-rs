use crate::constants::*;
use crate::errors::ErrorCode;
use crate::shared::math::{ceil_div, SqrtU128};
use anchor_lang::prelude::*;
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
        let numerator = (amount_out as u128)
            .checked_mul(reserve_in as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?;
        let amount_in = ceil_div(numerator, denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;
        Ok(amount_in)
    }
}

/// Constructs virtual reserves at pessimistic price = min(P_directional_ema, P_symmetric_ema) from spot reserves
/// - x_virt  = sqrt(k / P_pessimistic) [`collateral_ema_reserve`]
/// - y_virt = sqrt(k * P_pessimistic) [`debt_ema_reserve`]
pub fn construct_virtual_reserves_at_pessimistic_price(
    collateral_spot_reserve: u64,
    debt_spot_reserve: u64,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
) -> Result<(u64, u64)> {
    // Minimum liquidity check to prevent sqrt precision loss
    if collateral_spot_reserve < MIN_LIQUIDITY || debt_spot_reserve < MIN_LIQUIDITY {
        return Ok((0, 0));
    }

    let pessimistic_price = min(
        collateral_directional_ema_price_nad,
        collateral_ema_price_nad,
    ) as u128;
    if pessimistic_price == 0 {
        return Ok((collateral_spot_reserve, debt_spot_reserve));
    }

    let spot_k = (collateral_spot_reserve as u128)
        .checked_mul(debt_spot_reserve as u128)
        .ok_or(ErrorCode::Overflow)?;

    // k * NAD / P_pessimistic
    // Try direct multiplication first; on overflow, split as (R_c * NAD / P) * R_d
    // to keep intermediates within u128 (at a small precision cost).
    let x_virt_squared = match spot_k.checked_mul(NAD_U128) {
        Some(v) => v
            .checked_div(pessimistic_price)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = (collateral_spot_reserve as u128)
                .checked_mul(NAD_U128)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(pessimistic_price)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(debt_spot_reserve as u128)
                .ok_or(ErrorCode::Overflow)?
        }
    };
    // sqrt(k * NAD / P_pessimistic)
    let collateral_ema_reserve = x_virt_squared
        .sqrt()
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;

    // k * P_pessimistic / NAD
    // Try direct multiplication first; on overflow, split as (R_d * P / NAD) * R_c.
    let y_virt_squared = match spot_k.checked_mul(pessimistic_price) {
        Some(v) => v
            .checked_div(NAD_U128)
            .ok_or(ErrorCode::DenominatorOverflow)?,
        None => {
            let partial = (debt_spot_reserve as u128)
                .checked_mul(pessimistic_price)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(NAD_U128)
                .ok_or(ErrorCode::DenominatorOverflow)?;
            partial
                .checked_mul(collateral_spot_reserve as u128)
                .ok_or(ErrorCode::Overflow)?
        }
    };
    // sqrt(k * P_pessimistic / NAD)
    let debt_ema_reserve = y_virt_squared
        .sqrt()
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;

    Ok((collateral_ema_reserve, debt_ema_reserve))
}

/// Calculates collateral (X) needed to repay a given debt (Y) via AMM swap.
/// Answers: "How much X must be swapped to get `current_total_debt` Y out?"
/// Includes price impact from the constant product curve.
fn calculate_utilized_collateral_with_impact(
    current_total_debt: u64,
    collateral_amm_reserve: u64,
    debt_amm_reserve: u64,
    collateral_directional_ema_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u64> {
    let (collateral_ema_reserve, debt_ema_reserve) =
        construct_virtual_reserves_at_pessimistic_price(
            collateral_amm_reserve,
            debt_amm_reserve,
            collateral_ema_price_nad,
            collateral_directional_ema_price_nad,
        )?;

    CPCurve::calculate_amount_in(collateral_ema_reserve, debt_ema_reserve, current_total_debt)
}

/// Calculates the market's max total debt capacity given utilized + user collateral.
/// Includes price impact from the constant product curve.
/// Uses virtual reserves at min(directional_ema, ema) price to prevent manipulation.
fn calculate_max_allowed_total_debt(
    utilized_collateral: u64,
    user_collateral_amount: u64,
    collateral_amm_reserve: u64,
    debt_amm_reserve: u64,
    collateral_directional_ema_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u64> {
    let (collateral_ema_reserve, debt_ema_reserve) =
        construct_virtual_reserves_at_pessimistic_price(
            collateral_amm_reserve,
            debt_amm_reserve,
            collateral_ema_price_nad,
            collateral_directional_ema_price_nad,
        )?;

    let total_collateral_amount = utilized_collateral
        .checked_add(user_collateral_amount)
        .ok_or(ErrorCode::Overflow)?;
    CPCurve::calculate_amount_out(
        collateral_ema_reserve,
        debt_ema_reserve,
        total_collateral_amount,
    )
}

/// Maximum borrowable amount of tokenY using either a fixed CF or an impact-aware CF
///
/// Inputs:
/// - collateral_amount: X (raw collateral units)
/// - collateral_ema_price_nad: P_ema (NAD-scaled, Y/X)
/// - collateral_directional_ema_price_nad: P_directional_ema (NAD-scaled, Y/X) [~50 slots lagging behind inflated spot price]
/// - collateral_amm_reserve: R0 (raw X units)
/// - debt_amm_reserve: R1 (raw Y units)
/// - total_debt: existing total debt (raw Y units)
/// - fixed_cf_bps: Optional fixed collateral factor. If Some, uses this directly instead of AMM-based CF
///
/// Returns:
/// - final_borrow_limit (raw Y units)
/// - max_allowed_cf_bps (liquidation_cf_bps * 95%)
/// - liquidation_cf_bps
pub fn pessimistic_max_debt(
    collateral_amount: u64,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
    collateral_amm_reserve: u64,
    debt_amm_reserve: u64,
    total_debt: u64,
    fixed_cf_bps: Option<u16>,
) -> Result<(u64, u16, u16)> {
    // sanity checks
    if collateral_amount == 0
        || collateral_ema_price_nad == 0
        || collateral_directional_ema_price_nad == 0
    {
        return Ok((0, 0, 0));
    }

    // V_impact: impact-aware collateral value using virtual reserves at pessimistic price.
    // This matches the valuation used in liquidation (CPCurve::calculate_amount_out),
    // ensuring the borrow limit never exceeds the liquidation threshold.
    // Without this, V_linear > V_impact for collateral > ~5.26% of AMM reserve,
    // which would make max-borrow positions instantly liquidatable.
    let (collateral_ema_reserve, debt_ema_reserve) =
        construct_virtual_reserves_at_pessimistic_price(
            collateral_amm_reserve,
            debt_amm_reserve,
            collateral_ema_price_nad,
            collateral_directional_ema_price_nad,
        )?;

    let collateral_value_with_impact =
        CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, collateral_amount)?
            as u128;

    // Determine base CF: either fixed CF or dynamic AMM-based CF
    let base_cf_bps: u64 = if let Some(fixed_cf) = fixed_cf_bps {
        // Fixed CF path: use the fixed CF directly as base
        fixed_cf as u64
    } else {
        // Dynamic CF path: calculate impact-aware CF from AMM curve
        if debt_amm_reserve == 0 {
            return Ok((0, 0, 0));
        }

        // 0. Calculate utilized collateral with price impact using virtual reserves at pessimistic price.
        let utilized_collateral = calculate_utilized_collateral_with_impact(
            total_debt,
            collateral_amm_reserve,
            debt_amm_reserve,
            collateral_directional_ema_price_nad,
            collateral_ema_price_nad,
        )?;

        // 1. Calculate max allowed total debt using virtual reserves at pessimistic price.
        let max_allowed_total_debt = calculate_max_allowed_total_debt(
            utilized_collateral,
            collateral_amount,
            collateral_amm_reserve,
            debt_amm_reserve,
            collateral_directional_ema_price_nad,
            collateral_ema_price_nad,
        )?;

        // 2. Calculate user max debt.
        let user_max_debt = max_allowed_total_debt.checked_sub(total_debt).unwrap_or(0);

        // 3. Calculate base CF = user max debt * BPS_DENOMINATOR / V_impact
        //    CF is relative to impact value so it captures only the debt crowding effect.
        (user_max_debt as u128)
            .saturating_mul(BPS_DENOMINATOR_U128)
            .checked_div(collateral_value_with_impact)
            .unwrap_or(0) as u64
    };

    // Apply spot/EMA divergence cap to fixed cf only for preventing EMA lag front-running
    // CF_final = min(fixed_cf_bps, fixed_cf_bps * spot/ema)
    // fixed CF: capped at [100 bps, CF_final]
    // dynamic CF: capped at MAX_COLLATERAL_FACTOR_BPS bps
    let liquidation_cf_bps = if fixed_cf_bps.is_some() {
        // If spot > ema: CF stays at fixed_cf_bps
        // If spot < ema: CF reduces proportionally to render front-running non-profitable
        require!(
            collateral_ema_price_nad != 0,
            ErrorCode::DenominatorOverflow
        );
        let base = base_cf_bps as u128;
        let shrunk = (collateral_directional_ema_price_nad as u128)
            .saturating_mul(base)
            .checked_div(collateral_ema_price_nad as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        // Apply divergence cap: min(fixed_cf_bps, fixed_cf_bps * spot/ema)
        min(base, shrunk).max(100) as u16
    } else {
        // apply 85% maximum cap on dynamic CF
        // no need to apply divergence cap as base_cf_bps is based on impact with on virtual reserves at pessimistic price
        base_cf_bps.min(MAX_COLLATERAL_FACTOR_BPS as u64) as u16
    };

    // Max allowed CF BPS = liquidation CF * (1 - LTV_BUFFER_BPS / BPS_DENOMINATOR)
    // This creates a buffer between borrow limit and liquidation threshold
    let max_allowed_cf_bps = ((liquidation_cf_bps as u32)
        .saturating_mul((BPS_DENOMINATOR - LTV_BUFFER_BPS) as u32)
        / BPS_DENOMINATOR as u32) as u16;

    // Final borrow limit = V_impact * max_allowed_cf_bps / BPS
    // Uses impact-aware collateral value to match liquidation's valuation method,
    // guaranteeing borrow_limit ≤ liquidation_limit for any collateral size.
    let final_borrow_limit: u64 = collateral_value_with_impact
        .saturating_mul(max_allowed_cf_bps as u128)
        .checked_div(BPS_DENOMINATOR_U128)
        .unwrap_or(0)
        .min(u64::MAX as u128) as u64;

    Ok((final_borrow_limit, max_allowed_cf_bps, liquidation_cf_bps))
}
