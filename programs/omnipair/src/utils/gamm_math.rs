use anchor_lang::prelude::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::utils::math::{ceil_div, SqrtU128};
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
/// x_virt = sqrt(k / P_pessimistic), y_virt = sqrt(k * P_pessimistic)
/// x_virt = sqrt(k * NAD / P_pessimistic), y_virt = sqrt(k * P_pessimistic / NAD)
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
    
    let pessimistic_price = min(collateral_directional_ema_price_nad, collateral_ema_price_nad) as u128;
    if pessimistic_price == 0 {
        return Ok((collateral_spot_reserve, debt_spot_reserve));
    }

    let spot_k = (collateral_spot_reserve as u128)
    .checked_mul(debt_spot_reserve as u128)
    .ok_or(ErrorCode::Overflow)?;
    
    // k * NAD / P_pessimistic
    let x_virt_squared = spot_k
        .checked_mul(NAD_U128)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(pessimistic_price)
        .ok_or(ErrorCode::DenominatorOverflow)?;
    // sqrt(k * NAD / P_pessimistic)
    let x_virt = x_virt_squared
        .sqrt()
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;
    
    // k * P_pessimistic / NAD
    let y_virt_squared = spot_k
        .checked_mul(pessimistic_price)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(NAD_U128)
        .ok_or(ErrorCode::DenominatorOverflow)?;
    // sqrt(k * P_pessimistic / NAD)
    let y_virt = y_virt_squared
        .sqrt()
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?;
    
    Ok((x_virt, y_virt))
}

/// Calculates collateral (X) needed to repay a given debt (Y) via AMM swap.
/// Answers: "How much X must be swapped to get `current_total_debt` Y out?"
/// Includes price impact from the constant product curve.
fn calculate_utilized_collateral_with_impact(
    current_total_debt: u64, 
    collateral_amm_reserve: u64, 
    debt_amm_reserve: u64,
    collateral_spot_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u64> {
    let (x_virt, y_virt) = construct_virtual_reserves_at_pessimistic_price(
        collateral_amm_reserve,
        debt_amm_reserve,
        collateral_ema_price_nad,
        collateral_spot_price_nad,
    )?;
    
    CPCurve::calculate_amount_in(x_virt, y_virt, current_total_debt)
}

/// Calculates the pool's max total debt capacity given utilized + user collateral.
/// Includes price impact from the constant product curve.
/// Uses virtual reserves at min(spot, ema) price to prevent manipulation.
fn calculate_max_allowed_total_debt(
    utilized_collateral: u64, 
    user_collateral_amount: u64, 
    collateral_amm_reserve: u64, 
    debt_amm_reserve: u64,
    collateral_spot_price_nad: u64,
    collateral_ema_price_nad: u64,
) -> Result<u64> {
    let (x_virt, y_virt) = construct_virtual_reserves_at_pessimistic_price(
        collateral_amm_reserve,
        debt_amm_reserve,
        collateral_ema_price_nad ,
        collateral_spot_price_nad,
    )?;
    
    let total_collateral_amount = utilized_collateral.checked_add(user_collateral_amount).ok_or(ErrorCode::Overflow)?;
    CPCurve::calculate_amount_out(y_virt, x_virt, total_collateral_amount)
}

/// Maximum borrowable amount of tokenY using either a fixed CF or an impact-aware CF
///
/// Inputs:
/// - collateral_amount: X
/// - collateral_ema_price_scaled: P_ema (NAD-scaled, Y/X)
/// - collateral_directional_ema_price_scaled: P_directional_ema (NAD-scaled, Y/X) [~50 slots far from spot price, closer to ema price]
/// - debt_amm_reserve: R1 (raw Y units) - only used for dynamic CF calculation
/// - fixed_cf_bps: Optional fixed collateral factor. If Some, uses this directly instead of AMM-based CF
///
/// Returns:
/// - final_borrow_limit (NAD-scaled Y)
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

    // Collateral Value (V) in debt token (Y) = Collateral Amount (X) * Collateral EMA Price (P_ema) / NAD
    // V = X * P_ema / NAD  (NAD-scaled Y)
    let collateral_value = (collateral_amount as u128)
        .saturating_mul(collateral_ema_price_nad as u128)
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

        // 3. Calculate base CF = user max debt * BPS_DENOMINATOR / Collateral Value (V)
        (user_max_debt as u128)
        .saturating_mul(BPS_DENOMINATOR_U128)
        .checked_div(collateral_value) 
        .unwrap_or(0) as u64
    };

    // Apply spot/EMA divergence cap to fixed cf only for preventing EMA lag front-running
    // CF_final = min(fixed_cf_bps, fixed_cf_bps * spot/ema)
    // fixed CF: capped at [100 bps, CF_final]
    // dynamic CF: capped at MAX_COLLATERAL_FACTOR_BPS bps
    let liquidation_cf_bps = if fixed_cf_bps.is_some() {
        // If spot > ema: CF stays at fixed_cf_bps
        // If spot < ema: CF reduces proportionally to render front-running non-profitable
        require!(collateral_ema_price_nad != 0, ErrorCode::DenominatorOverflow);
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

    // Final borrow limit = V * max_allowed_cf_bps / BPS
    let final_borrow_limit: u64 = collateral_value
        .saturating_mul(max_allowed_cf_bps as u128)
        .checked_div(BPS_DENOMINATOR_U128)
        .unwrap_or(0)
        .min(u64::MAX as u128) as u64;

    Ok((final_borrow_limit, max_allowed_cf_bps, liquidation_cf_bps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // =======
    // HELPER FUNCTIONS
    
    fn spot_price_nad(collateral_reserve: u64, debt_reserve: u64) -> u64 {
        if collateral_reserve == 0 {
            return 0;
        }
        ((debt_reserve as u128 * NAD_U128) / collateral_reserve as u128) as u64
    }

    /// Compute max borrowable debt for a user given pool state (raw, before CF scaling).
    fn compute_user_max_debt_raw(
        user_collateral: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        existing_total_debt: u64,
    ) -> u64 {
        if existing_total_debt >= debt_reserve {
            return 0;
        }
        let price = spot_price_nad(collateral_reserve, debt_reserve);
        let utilized_collateral = calculate_utilized_collateral_with_impact(
            existing_total_debt, collateral_reserve, debt_reserve, price, price,
        ).unwrap();
        let max_total_debt = calculate_max_allowed_total_debt(
            utilized_collateral, user_collateral, collateral_reserve, debt_reserve, price, price,
        ).unwrap();
        max_total_debt.saturating_sub(existing_total_debt)
    }

    /// Check that split accounts cannot borrow more than a single account.
    fn check_split_attack_mitigated(
        total_collateral: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
        num_splits: u64,
    ) {
        let single_max = compute_user_max_debt_raw(
            total_collateral, collateral_reserve, debt_reserve, 0,
        );
        
        let split_collateral = total_collateral / num_splits;
        let mut accumulated_debt = 0u64;
        let mut split_total = 0u64;
        
        for _ in 0..num_splits {
            let borrow = compute_user_max_debt_raw(
                split_collateral, collateral_reserve, debt_reserve, accumulated_debt,
            );
            split_total += borrow;
            accumulated_debt += borrow;
        }
        
        assert!(
            split_total <= single_max,
            "VULNERABILITY: Split {} accounts ({}) > single ({})",
            num_splits, split_total, single_max
        );
    }

    /// Check that AMM invariant (x * y = k) is preserved or improved after swap.
    fn check_amm_invariant_preserved(
        reserve_in: u64,
        reserve_out: u64,
        amount_in: u64,
    ) {
        let k_before = (reserve_in as u128) * (reserve_out as u128);
        
        if let Ok(amount_out) = CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in) {
            let new_reserve_in = reserve_in as u128 + amount_in as u128;
            let new_reserve_out = (reserve_out as u128).saturating_sub(amount_out as u128);
            let k_after = new_reserve_in * new_reserve_out;
            
            assert!(
                k_after >= k_before,
                "AMM invariant violated: k_before={}, k_after={}", k_before, k_after
            );
        }
    }

    /// Check that utilized collateral increases monotonically with debt.
    fn check_utilized_collateral_monotonic(
        debt_1: u64,
        debt_2: u64,
        collateral_reserve: u64,
        debt_reserve: u64,
    ) {
        if debt_1 >= debt_reserve || debt_2 >= debt_reserve {
            return;
        }
        
        let price = spot_price_nad(collateral_reserve, debt_reserve);
        let utilized_1 = calculate_utilized_collateral_with_impact(
            debt_1, collateral_reserve, debt_reserve, price, price,
        ).unwrap_or(0);
        let utilized_2 = calculate_utilized_collateral_with_impact(
            debt_2, collateral_reserve, debt_reserve, price, price,
        ).unwrap_or(0);
        
        if debt_1 < debt_2 {
            assert!(utilized_1 <= utilized_2, "Utilized should increase with debt");
        } else if debt_1 > debt_2 {
            assert!(utilized_1 >= utilized_2, "Utilized should decrease with less debt");
        }
    }

    // =======
    // DETERMINISTIC TESTS - Exact value verification

    #[test]
    fn cpcurve_amount_out() {
        // (100 * 1000) / (1000 + 100) = 90
        let out = CPCurve::calculate_amount_out(1000, 1000, 100).unwrap();
        assert_eq!(out, 90);
        println!("amount_out = (100 * 1000) / 1100 = {}", out);
    }

    #[test]
    fn cpcurve_amount_in() {
        // ceil((90 * 1000) / (1000 - 90)) = ceil(90000 / 910) = 99
        let amount_in = CPCurve::calculate_amount_in(1000, 1000, 90).unwrap();
        assert_eq!(amount_in, 99);
        println!("amount_in = ceil(90 * 1000 / 910) = {}", amount_in);
    }

    #[test]
    fn split_attack_two_way() {
        let (cr, dr) = (1_000_000, 1_000_000);
        
        // Single: 1M * 1M / 2M = 500k
        let single = compute_user_max_debt_raw(1_000_000, cr, dr, 0);
        assert_eq!(single, 500_000);

        // Split #1: 500k * 1M / 1.5M = 333,333
        let first = compute_user_max_debt_raw(500_000, cr, dr, 0);
        assert_eq!(first, 333_333);

        // Split #2: value depends on rounding behavior
        let second = compute_user_max_debt_raw(500_000, cr, dr, first);

        let split_total = first + second;
        
        // Key invariant: Split attack is still mitigated
        assert!(split_total <= single, "Split attack should not exceed single");

        println!("=== 2-Way Split Attack ===");
        println!("Single (1M collateral): = {}", single);
        println!("Split #1 (500k): = {}", first);
        println!("Split #2 (500k): = {}", second);
        println!("Split total: {} | Diff from single: {}", split_total, single as i64 - split_total as i64);
    }

    #[test]
    fn split_attack_three_way() {
        let (cr, dr) = (1_000_000, 1_000_000);
        
        // Single: 900k * 1M / 1.9M = 473,684
        let single = compute_user_max_debt_raw(900_000, cr, dr, 0);
        assert_eq!(single, 473_684);

        // #1: 300k * 1M / 1.3M = 230,769
        let b1 = compute_user_max_debt_raw(300_000, cr, dr, 0);
        assert_eq!(b1, 230_769);

        // #2 and #3: values depend on rounding behavior
        let b2 = compute_user_max_debt_raw(300_000, cr, dr, b1);
        let b3 = compute_user_max_debt_raw(300_000, cr, dr, b1 + b2);

        let split_total = b1 + b2 + b3;
        
        // Key invariant: Split attack is still mitigated
        assert!(split_total <= single, "Split attack should not exceed single");

        println!("=== 3-Way Split Attack ===");
        println!("Single: {} | Split: {}+{}+{} = {}", single, b1, b2, b3, split_total);
    }

    #[test]
    fn utilized_collateral_values() {
        let (cr, dr) = (1_000_000, 1_000_000);
        let price = spot_price_nad(cr, dr);
        // util = ceil(debt * cr / (dr - debt))
        
        let u0 = calculate_utilized_collateral_with_impact(0, cr, dr, price, price).unwrap();       // 0
        let u100k = calculate_utilized_collateral_with_impact(100_000, cr, dr, price, price).unwrap(); // ceil(100k*1M/900k) = 111,112
        let u200k = calculate_utilized_collateral_with_impact(200_000, cr, dr, price, price).unwrap(); // ceil(200k*1M/800k) = 250,000
        
        assert_eq!(u0, 0);
        assert_eq!(u100k, 111_112); // ceil instead of floor
        assert_eq!(u200k, 250_000);

        println!("=== Utilized Collateral ===");
        println!("debt=0→{} | 100k→{} | 200k→{}", u0, u100k, u200k);
        println!("+100k debt (100k→200k) = +{} utilized", u200k - u100k);
    }

    #[test]
    fn max_debt_decreases_with_pool_debt() {
        let (cr, dr, uc) = (1_000_000, 1_000_000, 100_000);
        
        // @0: 100k * 1M / 1.1M = 90,909
        let max_0 = compute_user_max_debt_raw(uc, cr, dr, 0);
        assert_eq!(max_0, 90_909);
        
        // @200k: util=250k, total=350k, max_total=259,259, user=59,259
        let max_200k = compute_user_max_debt_raw(uc, cr, dr, 200_000);
        assert_eq!(max_200k, 59_259);
        
        // @500k: util=1M, total=1.1M, max_total=523,809, user=23,809
        let max_500k = compute_user_max_debt_raw(uc, cr, dr, 500_000);
        assert_eq!(max_500k, 23_809);

        println!("=== Max Debt vs Pool Debt (100k collateral) ===");
        println!("pool=0→{} | 200k→{} | 500k→{}", max_0, max_200k, max_500k);
    }

    #[test]
    fn pessimistic_max_debt_values() {
        // user_max=500k, cf=500k*10000/1M=5000bps, max_cf=4750, limit=475k
        let (limit, max_cf, liq_cf) = pessimistic_max_debt(
            1_000_000, NAD, NAD, 1_000_000, 1_000_000, 0, None
        ).unwrap();
        
        assert_eq!((liq_cf, max_cf, limit), (5000, 4750, 475_000));

        println!("=== Pessimistic Max Debt (1M coll, 0 debt) ===");
        println!("liq_cf={} bps | max_cf={} bps | limit={}", liq_cf, max_cf, limit);
    }

    #[test]
    fn pessimistic_max_debt_with_existing_debt_values() {
        // @0: user_max=333,333, cf=6666bps, max_cf=6332, limit=316,600
        let (l0, cf0, _) = pessimistic_max_debt(500_000, NAD, NAD, 1_000_000, 1_000_000, 0, None).unwrap();
        assert_eq!((l0, cf0), (316_600, 6332));
        
        // @200k: user_max=228,571, cf=4571bps, max_cf=4342, limit=217,100
        let (l200k, cf200k, _) = pessimistic_max_debt(500_000, NAD, NAD, 1_000_000, 1_000_000, 200_000, None).unwrap();
        assert_eq!((l200k, cf200k), (217_100, 4342));
        
        assert_eq!(l0 - l200k, 99_500);

        println!("=== Pessimistic Max Debt with Existing Debt ===");
        println!("@0: cf={}, limit={} | @200k: cf={}, limit={}", cf0, l0, cf200k, l200k);
    }

    #[test]
    fn interest_accrual_scenario() {
        let (cr, dr, uc) = (1_000_000, 1_000_000, 500_000);
        let price = spot_price_nad(cr, dr);
        
        // Initial: 500k * 1M / 1.5M = 333,333
        let initial = compute_user_max_debt_raw(uc, cr, dr, 0);
        assert_eq!(initial, 333_333);
        
        let first_borrow = initial / 2; // 166,666
        assert_eq!(first_borrow, 166_666);
        
        // After borrow: remaining capacity
        let remaining_after = compute_user_max_debt_raw(uc, cr, dr, first_borrow);
        
        // 10% interest: 16,666
        let interest = first_borrow * 1000 / 10_000;
        assert_eq!(interest, 16_666);
        
        let pool_debt_after = first_borrow + interest; // 183,332
        
        // After interest: remaining decreases
        let remaining_after_interest = compute_user_max_debt_raw(uc, cr, dr, pool_debt_after);
        assert!(remaining_after_interest < remaining_after);
        
        // Utilized increases with more debt
        let util_before = calculate_utilized_collateral_with_impact(first_borrow, cr, dr, price, price).unwrap();
        let util_after = calculate_utilized_collateral_with_impact(pool_debt_after, cr, dr, price, price).unwrap();
        assert!(util_after > util_before);

        println!("=== Interest Accrual ===");
        println!("Borrow: {} | Interest: {} | Pool debt: {}", first_borrow, interest, pool_debt_after);
        println!("Remaining: {} → {} (Δ={})", remaining_after, remaining_after_interest, remaining_after - remaining_after_interest);
        println!("Utilized: {} → {} (+{})", util_before, util_after, util_after - util_before);
    }

    // =======
    // VIRTUAL RESERVES INVARIANTS
    // Verifies pessimistic pricing: virtual reserves at min(P_spot, P_ema) preserve k.

    #[test]
    fn virtual_reserves_preserves_invariant() {
        // Spot: (800 NAD, 625 NAD), P_spot=0.78125 | EMA: P_ema=0.5
        let (x_spot, y_spot) = (800 * NAD, 625 * NAD);
        let p_spot_nad = (625 * NAD) / 800;
        let p_ema_nad = NAD / 2;
        
        let (x_virt, y_virt) = construct_virtual_reserves_at_pessimistic_price(
            x_spot, y_spot, p_ema_nad, p_spot_nad
        ).unwrap();
        
        // Virtual reserves at P_safe=min(0.78125, 0.5)=0.5 → (1000 NAD, 500 NAD)
        assert_eq!((x_virt, y_virt), (1000 * NAD, 500 * NAD));
        
        // Invariant: k preserved
        let k_spot = x_spot as u128 * y_spot as u128;
        let k_virt = x_virt as u128 * y_virt as u128;
        assert_eq!(k_spot, k_virt);
    }

    #[test]
    fn utilized_collateral_uses_pessimistic_price() {
        // Spot (800 NAD, 625 NAD) with P_spot=0.78125, P_ema=0.5, debt=100 NAD
        // Without protection: 100*800/(625-100) ≈ 152 NAD
        // With protection (virtual 1000, 500): 100*1000/(500-100) = 250 NAD
        let (x_spot, y_spot, debt) = (800 * NAD, 625 * NAD, 100 * NAD);
        let p_spot_nad = (625 * NAD) / 800;
        let p_ema_nad = NAD / 2;
        
        let utilized = calculate_utilized_collateral_with_impact(
            debt, x_spot, y_spot, p_spot_nad, p_ema_nad
        ).unwrap();
        
        let utilized_fair = calculate_utilized_collateral_with_impact(
            debt, 1000 * NAD, 500 * NAD, p_ema_nad, p_ema_nad
        ).unwrap();
        
        assert_eq!(utilized, utilized_fair);
        assert_eq!(utilized, 250 * NAD);
    }

    #[test]
    fn max_debt_bounded_by_fair_price() {
        // Fair: (1000 NAD, 1000 NAD), P=1.0 | Manipulated: (800 NAD, 1250 NAD), P_spot=1.5625
        let (user_coll, pool_debt) = (100 * NAD, 50 * NAD);
        let (x_fair, y_fair) = (1000 * NAD, 1000 * NAD);
        let (x_manip, y_manip) = (800 * NAD, 1250 * NAD);
        let p_ema_nad = NAD;
        let p_spot_nad = (y_manip as u128 * NAD as u128 / x_manip as u128) as u64;
        
        let price_fair = spot_price_nad(x_fair, y_fair);
        let util_fair = calculate_utilized_collateral_with_impact(pool_debt, x_fair, y_fair, price_fair, price_fair).unwrap();
        let max_fair = calculate_max_allowed_total_debt(util_fair, user_coll, x_fair, y_fair, price_fair, price_fair)
            .unwrap().saturating_sub(pool_debt);
        
        let util_manip = calculate_utilized_collateral_with_impact(pool_debt, x_manip, y_manip, p_spot_nad, p_ema_nad).unwrap();
        let max_manip = calculate_max_allowed_total_debt(util_manip, user_coll, x_manip, y_manip, p_spot_nad, p_ema_nad)
            .unwrap().saturating_sub(pool_debt);
        
        // Invariant: manipulation cannot increase borrowing capacity
        assert!(max_manip <= max_fair);
    }

    // =======
    // PROPERTY-BASED TESTS - Invariants that must always hold

    proptest! {
        #[test]
        fn amm_invariant_never_decreases(
            reserve_in in 1_000u64..1_000_000_000,
            reserve_out in 1_000u64..1_000_000_000,
            amount_in in 1u64..100_000_000,
        ) {
            check_amm_invariant_preserved(reserve_in, reserve_out, amount_in);
        }
    }

    proptest! {
        #[test]
        fn split_never_exceeds_single(
            total_collateral in 100_000u64..10_000_000,
            reserve in 1_000_000u64..100_000_000,
            num_splits in 2u64..5,
        ) {
            // Ensure collateral is divisible by splits
            let total_collateral = (total_collateral / num_splits) * num_splits;
            prop_assume!(total_collateral > 0);
            prop_assume!(total_collateral < reserve); // Must be less than reserve
            
            check_split_attack_mitigated(total_collateral, reserve, reserve, num_splits);
        }
    }

    proptest! {
        #[test]
        fn utilized_collateral_monotonic(
            debt_1 in 0u64..500_000,
            debt_2 in 0u64..500_000,
            reserve in 1_000_000u64..10_000_000,
        ) {
            check_utilized_collateral_monotonic(debt_1, debt_2, reserve, reserve);
        }
    }

    proptest! {
        #[test]
        fn max_debt_decreases_with_existing_debt(
            user_collateral in 10_000u64..500_000,
            reserve in 1_000_000u64..10_000_000,
            existing_debt_1 in 0u64..300_000,
            existing_debt_2 in 0u64..300_000,
        ) {
            prop_assume!(existing_debt_1 < reserve && existing_debt_2 < reserve);
            
            let max_1 = compute_user_max_debt_raw(user_collateral, reserve, reserve, existing_debt_1);
            let max_2 = compute_user_max_debt_raw(user_collateral, reserve, reserve, existing_debt_2);
            
            if existing_debt_1 < existing_debt_2 {
                assert!(max_1 >= max_2, "Max debt should decrease as pool debt increases");
            }
        }
    }

    proptest! {
        #[test]
        fn amount_out_never_exceeds_reserve(
            reserve_in in 1_000u64..1_000_000_000,
            reserve_out in 1_000u64..1_000_000_000,
            amount_in in 1u64..1_000_000_000,
        ) {
            if let Ok(amount_out) = CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in) {
                assert!(amount_out < reserve_out, "Amount out must be less than reserve");
            }
        }
    }

    proptest! {
        #[test]
        fn manipulation_bounded_by_ema(
            collateral_reserve in 100_000u64..10_000_000,
            debt_reserve in 100_000u64..10_000_000,
            user_collateral in 10_000u64..100_000,
            existing_debt in 0u64..50_000,
            manipulation_pct in 10u64..100,
        ) {
            prop_assume!(existing_debt < debt_reserve);
            
            let p_ema = spot_price_nad(collateral_reserve, debt_reserve);
            
            // Inflate price by manipulation_pct%, preserving k
            let factor = 100 + manipulation_pct;
            let sqrt_factor = ((factor as u128 * 100).sqrt().unwrap()) as u64;
            let x_manip = (collateral_reserve as u128 * 100 / sqrt_factor as u128) as u64;
            let y_manip = (debt_reserve as u128 * sqrt_factor as u128 / 100) as u64;
            
            prop_assume!(x_manip > 0 && y_manip > 0 && existing_debt < y_manip);
            
            let p_spot = spot_price_nad(x_manip, y_manip);
            
            let util_fair = calculate_utilized_collateral_with_impact(
                existing_debt, collateral_reserve, debt_reserve, p_ema, p_ema
            ).unwrap();
            let max_fair = calculate_max_allowed_total_debt(
                util_fair, user_collateral, collateral_reserve, debt_reserve, p_ema, p_ema
            ).unwrap().saturating_sub(existing_debt);
            
            let util_manip = calculate_utilized_collateral_with_impact(
                existing_debt, x_manip, y_manip, p_spot, p_ema
            ).unwrap();
            let max_manip = calculate_max_allowed_total_debt(
                util_manip, user_collateral, x_manip, y_manip, p_spot, p_ema
            ).unwrap().saturating_sub(existing_debt);
            
            // Invariant: max_debt(manipulated) ≤ max_debt(fair)
            assert!(max_manip <= max_fair);
        }
    }
}
