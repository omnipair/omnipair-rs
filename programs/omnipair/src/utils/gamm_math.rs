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
            // TODO: Use ceil_div instead of floor to round in favor of the protocol.
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // =======
    // HELPER FUNCTIONS
    
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
        let utilized_collateral = calculate_utilized_collateral_with_impact(
            existing_total_debt, collateral_reserve, debt_reserve,
        ).unwrap();
        let max_total_debt = calculate_max_allowed_total_debt(
            utilized_collateral, user_collateral, collateral_reserve, debt_reserve,
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
        
        let utilized_1 = calculate_utilized_collateral_with_impact(
            debt_1, collateral_reserve, debt_reserve,
        ).unwrap_or(0);
        let utilized_2 = calculate_utilized_collateral_with_impact(
            debt_2, collateral_reserve, debt_reserve,
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
        // (90 * 1000) / (1000 - 90) = 98
        let amount_in = CPCurve::calculate_amount_in(1000, 1000, 90).unwrap();
        assert_eq!(amount_in, 98);
        println!("amount_in = (90 * 1000) / 910 = {}", amount_in);
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

        // Split #2: util=499,999, max_total=499,999, user=166,666
        let second = compute_user_max_debt_raw(500_000, cr, dr, first);
        assert_eq!(second, 166_666);

        let split_total = first + second;
        assert_eq!(split_total, 499_999);
        
        let rounding_loss = single - split_total; // 500,000 - 499,999 = 1
        assert_eq!(rounding_loss, 1);

        println!("=== 2-Way Split Attack ===");
        println!("Single (1M collateral): = {}", single);
        println!("Split #1 (500k): = {}", first);
        println!("Split #2 (500k): = {}", second);
        println!("Split total: {} | Rounding loss: {}", split_total, rounding_loss);
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

        // #2: util=299,999, max_total=374,999, user=144,230
        let b2 = compute_user_max_debt_raw(300_000, cr, dr, b1);
        assert_eq!(b2, 144_230);

        // #3: util=599,998, max_total=473,683, user=98,684
        let b3 = compute_user_max_debt_raw(300_000, cr, dr, b1 + b2);
        assert_eq!(b3, 98_684);

        let split_total = b1 + b2 + b3;
        assert_eq!(split_total, 473_683);
        assert_eq!(single - split_total, 1); // rounding loss

        println!("=== 3-Way Split Attack ===");
        println!("Single: {} | Split: {}+{}+{} = {}", single, b1, b2, b3, split_total);
    }

    #[test]
    fn utilized_collateral_values() {
        let (cr, dr) = (1_000_000, 1_000_000);
        // util = debt * cr / (dr - debt)
        
        let u0 = calculate_utilized_collateral_with_impact(0, cr, dr).unwrap();       // 0
        let u100k = calculate_utilized_collateral_with_impact(100_000, cr, dr).unwrap(); // 100k*1M/900k = 111,111
        let u200k = calculate_utilized_collateral_with_impact(200_000, cr, dr).unwrap(); // 200k*1M/800k = 250,000
        
        assert_eq!((u0, u100k, u200k), (0, 111_111, 250_000));
        assert_eq!(u200k - u100k, 138_889); // non-linear: +100k debt → +138k util

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
        
        // Initial: 500k * 1M / 1.5M = 333,333
        let initial = compute_user_max_debt_raw(uc, cr, dr, 0);
        assert_eq!(initial, 333_333);
        
        let first_borrow = initial / 2; // 166,666
        assert_eq!(first_borrow, 166_666);
        
        // After borrow: util=199,999, remaining=245,098
        let remaining_after = compute_user_max_debt_raw(uc, cr, dr, first_borrow);
        assert_eq!(remaining_after, 245_098);
        
        // 10% interest: 16,666
        let interest = first_borrow * 1000 / 10_000;
        assert_eq!(interest, 16_666);
        
        let pool_debt_after = first_borrow + interest; // 183,332
        
        // After interest: util=224,487, remaining=236,785
        let remaining_after_interest = compute_user_max_debt_raw(uc, cr, dr, pool_debt_after);
        assert_eq!(remaining_after_interest, 236_785);
        assert_eq!(remaining_after - remaining_after_interest, 8_313); // capacity reduction
        
        // Utilized: 199,999 → 224,487 (+24,488, 1.47x interest)
        let util_before = calculate_utilized_collateral_with_impact(first_borrow, cr, dr).unwrap();
        let util_after = calculate_utilized_collateral_with_impact(pool_debt_after, cr, dr).unwrap();
        assert_eq!((util_before, util_after), (199_999, 224_487));
        assert_eq!(util_after - util_before, 24_488);

        println!("=== Interest Accrual ===");
        println!("Borrow: {} | Interest: {} | Pool debt: {}", first_borrow, interest, pool_debt_after);
        println!("Remaining: {} → {} (Δ={})", remaining_after, remaining_after_interest, remaining_after - remaining_after_interest);
        println!("Utilized: {} → {} (+{}, {:.2}x)", util_before, util_after, util_after - util_before, 
            (util_after - util_before) as f64 / interest as f64);
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
}
