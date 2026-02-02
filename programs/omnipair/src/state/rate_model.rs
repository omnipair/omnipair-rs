use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::math::*;

#[account]
#[derive(InitSpace)]
pub struct RateModel {
    /// exp_rate: NAD/millisecond (k_real = exp_rate / NAD)
    /// Derived from half_life_ms: exp_rate = ln(2) / half_life_ms
    pub exp_rate: u64,
    /// utilization band edges (NAD-scaled: 0..NAD)
    pub target_util_start: u64,
    pub target_util_end: u64,
    /// Rate adjustment half-life in milliseconds
    /// Controls how fast rates adjust to utilization changes
    /// Lower = faster adjustments, Higher = slower adjustments
    pub half_life_ms: u64,
    /// Minimum interest rate floor (NAD-scaled)
    /// Rate will not drop below this value
    pub min_rate: u64,
    /// Maximum interest rate ceiling (NAD-scaled)
    /// Rate will not exceed this value (0 = no cap)
    pub max_rate: u64,
    /// Initial interest rate for new pairs using this model (NAD-scaled)
    pub initial_rate: u64,
}

impl RateModel {
    /// Creates a new RateModel with fully configurable parameters
    /// 
    /// # Arguments
    /// * `target_util_start_bps` - Lower bound of optimal utilization range (bps)
    /// * `target_util_end_bps` - Upper bound of optimal utilization range (bps)
    /// * `half_life_ms` - Rate adjustment half-life in milliseconds (controls speed)
    /// * `min_rate_bps` - Minimum rate floor (bps)
    /// * `max_rate_bps` - Maximum rate ceiling (bps, 0 = no cap)
    /// * `initial_rate_bps` - Starting rate for pairs using this model (bps)
    pub fn new(
        target_util_start_bps: u64,
        target_util_end_bps: u64,
        half_life_ms: u64,
        min_rate_bps: u64,
        max_rate_bps: u64,
        initial_rate_bps: u64,
    ) -> Self {
        // Calculate exp_rate from half_life: exp_rate = ln(2) / half_life_ms
        let exp_rate = NATURAL_LOG_OF_TWO_NAD / half_life_ms;
        
        Self {
            exp_rate,
            target_util_start: Self::bps_to_nad(target_util_start_bps),
            target_util_end: Self::bps_to_nad(target_util_end_bps),
            half_life_ms,
            min_rate: Self::bps_to_nad(min_rate_bps),
            max_rate: if max_rate_bps == 0 { 0 } else { Self::bps_to_nad(max_rate_bps) },
            initial_rate: Self::bps_to_nad(initial_rate_bps),
        }
    }

    /// Validates that utilization bounds are valid:
    /// - start < end
    /// - both within [100, 10000] bps
    /// - start >= MIN_TARGET_UTIL_BPS (e.g., 1%)
    /// - end <= MAX_TARGET_UTIL_BPS (e.g., 100%)
    pub fn validate_util_bounds(start_bps: u64, end_bps: u64) -> bool {
        start_bps < end_bps 
            && start_bps >= MIN_TARGET_UTIL_BPS 
            && end_bps <= MAX_TARGET_UTIL_BPS
    }
    
    /// Validates rate model parameters
    /// - half_life_ms within [MIN_RATE_HALF_LIFE_MS, MAX_RATE_HALF_LIFE_MS]
    /// - min_rate_bps <= max_rate_bps (if max is set)
    /// - initial_rate_bps within [min_rate_bps, max_rate_bps] (if max is set)
    /// - all values within allowed bounds
    pub fn validate_rate_params(
        half_life_ms: u64,
        min_rate_bps: u64,
        max_rate_bps: u64,
        initial_rate_bps: u64,
    ) -> bool {
        // Validate half-life bounds
        if half_life_ms < MIN_RATE_HALF_LIFE_MS || half_life_ms > MAX_RATE_HALF_LIFE_MS {
            return false;
        }
        
        // Validate rate bounds
        if min_rate_bps > MAX_ALLOWED_RATE_BPS {
            return false;
        }
        
        // If max_rate is set (non-zero), validate it
        if max_rate_bps > 0 {
            if max_rate_bps > MAX_ALLOWED_RATE_BPS {
                return false;
            }
            if min_rate_bps > max_rate_bps {
                return false;
            }
        }
        
        // Validate initial rate bounds
        if initial_rate_bps < MIN_INITIAL_RATE_BPS || initial_rate_bps > MAX_INITIAL_RATE_BPS {
            return false;
        }
        
        // Initial rate should be >= min_rate
        if initial_rate_bps < min_rate_bps {
            return false;
        }
        
        // If max is set, initial rate should be <= max_rate
        if max_rate_bps > 0 && initial_rate_bps > max_rate_bps {
            return false;
        }
        
        true
    }

    /// Returns (current_rate_NAD, integral_NAD) where:
    /// - current_rate_NAD is APR in NAD
    /// - integral_NAD = rate * (dt / YEAR) in NAD, suitable for: interest = debt * integral / NAD
    pub fn calculate_rate(&self, last_rate: u64, time_elapsed: u64, last_util: u64) -> (u64, u64) {
        let dt = time_elapsed as u128;
        if dt == 0 {
            return (last_rate, 0);
        }

        // constants & helpers
        let exp_rate = self.exp_rate as u128;                     // NAD/ms
        let x        = exp_rate.saturating_mul(dt);               // NAD (k*dt in NAD units)
        let gd       = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS) as u128; // NAD ≈ e^{-(x/NAD)}

        // Use configurable min_rate (NAD-scaled)
        let min_nad  = self.min_rate as u128;
        // Use configurable max_rate (NAD-scaled), 0 = no cap
        let max_nad  = self.max_rate as u128;
        let has_max_cap = max_nad > 0;

        // Enforce the MIN floor
        let last = (last_rate as u128).max(min_nad);

        // High util: exponential growth (with optional MAX cap)
        if (last_util as u128) > (self.target_util_end as u128) {
            // r1 = r0 * e^{+k dt} = r0 * NAD / gd
            let curr_unclamped = last
                .saturating_mul(NAD as u128)
                / gd.max(1);

            // Apply max cap if configured
            let curr = if has_max_cap && curr_unclamped > max_nad {
                // Hit MAX during window → split: exponential up to MAX, then flat MAX
                if last >= max_nad {
                    // Already at max, stay flat
                    let integral = ceil_div(max_nad.saturating_mul(dt), MILLISECONDS_PER_YEAR as u128)
                        .unwrap_or(max_nad.saturating_mul(dt) / (MILLISECONDS_PER_YEAR as u128));
                    return (max_nad.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
                }
                let t_to_max = Self::time_to_reach_closed_form(last, max_nad, exp_rate, /*up=*/true)
                    .min(dt);
                
                // exp part (to ceiling): (MAX - last) * NAD / exp_rate
                let exp_part = ceil_div(max_nad.saturating_sub(last).saturating_mul(NAD as u128), exp_rate)
                    .unwrap_or(max_nad.saturating_sub(last).saturating_mul(NAD as u128) / exp_rate);
                // flat tail: MAX * (dt - t*)
                let flat_part = max_nad.saturating_mul(dt.saturating_sub(t_to_max));
                let integral = ceil_div(exp_part + flat_part, MILLISECONDS_PER_YEAR as u128)
                    .unwrap_or((exp_part + flat_part) / (MILLISECONDS_PER_YEAR as u128));
                return (max_nad.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
            } else {
                curr_unclamped
            };

            // ∫ r dt = (r1 - r0) / k_real = (r1 - r0) * NAD / exp_rate, then / YEAR
            let numer    = curr.saturating_sub(last).saturating_mul(NAD as u128);
            let integral_pre = numer / exp_rate;
            let integral = ceil_div(integral_pre, MILLISECONDS_PER_YEAR as u128).unwrap_or(integral_pre / (MILLISECONDS_PER_YEAR as u128));
            return (curr.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
        }

        // Low util: exponential decay with MIN floor
        if (last_util as u128) < (self.target_util_start as u128) {
            // r1 = r0 * e^{-k dt} = r0 * gd / NAD
            let r1_unclamped = last.saturating_mul(gd) / (NAD as u128);

            if r1_unclamped >= min_nad {
                let curr = r1_unclamped;
                // ∫ = (r0 - r1) * NAD / exp_rate, then / YEAR
                let numer    = last.saturating_sub(curr).saturating_mul(NAD as u128);
                let integral_pre = numer / exp_rate;
                let integral = ceil_div(integral_pre, MILLISECONDS_PER_YEAR as u128).unwrap_or(integral_pre / (MILLISECONDS_PER_YEAR as u128));
                return (curr.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
            } else {
                // Hit MIN during window → split: exponential down to MIN, then flat MIN
                if last <= min_nad {
                    let integral = ceil_div(min_nad.saturating_mul(dt), MILLISECONDS_PER_YEAR as u128)
                        .unwrap_or(min_nad.saturating_mul(dt) / (MILLISECONDS_PER_YEAR as u128));
                    return (min_nad.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
                }
                let t_to_min = Self::time_to_reach_closed_form(last, min_nad, exp_rate, /*up=*/false)
                    .min(dt);

                // exp part (to floor): (last - MIN) * NAD / exp_rate
                let exp_part  = ceil_div(last.saturating_sub(min_nad).saturating_mul(NAD as u128), exp_rate)
                    .unwrap_or(last.saturating_sub(min_nad).saturating_mul(NAD as u128) / exp_rate);
                // flat tail: MIN * (dt - t*)
                let flat_part = min_nad.saturating_mul(dt.saturating_sub(t_to_min));
                let integral  = ceil_div(exp_part + flat_part, MILLISECONDS_PER_YEAR as u128)
                    .unwrap_or((exp_part + flat_part) / (MILLISECONDS_PER_YEAR as u128));
                return (min_nad.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64);
            }
        }

        // Middle band: flat
        let integral = ceil_div(last.saturating_mul(dt), MILLISECONDS_PER_YEAR as u128)
            .unwrap_or(last.saturating_mul(dt) / (MILLISECONDS_PER_YEAR as u128));
        (last.min(u64::MAX as u128) as u64, integral.min(u64::MAX as u128) as u64)
    }

    /// Closed-form time to reach target using ln.
    /// up=false : r(t) = r0 * e^{-k t} <= target  ⇒  t = ln(r0/target) / k = ln(r0/target) * NAD / exp_rate
    /// up=true  : r(t) = r0 * e^{+k t} >= target  ⇒  t = ln(target/r0) / k
    /// Since ln_ratio is already NAD-scaled, t = ln_ratio / exp_rate (no extra NAD multiplier needed)
    #[inline]
    fn time_to_reach_closed_form(r0: u128, target: u128, exp_rate: u128, up: bool) -> u128 {
        if up {
            // Used when max_rate cap is configured
            if target <= r0 { return 0; }
            let ratio_nad = (target.saturating_mul(NAD as u128)) / r0.max(1);
            let ln_ratio  = Self::ln_nad(ratio_nad as u64);  // NAD (signed)
            let t = ln_ratio / (exp_rate as i128);
            if t <= 0 { 0 } else { t as u128 }
        } else {
            if r0 <= target { return 0; }
            let ratio_nad = (r0.saturating_mul(NAD as u128)) / target.max(1);
            let ln_ratio  = Self::ln_nad(ratio_nad as u64);  // NAD (signed)
            let t = ln_ratio / (exp_rate as i128);
            if t <= 0 { 0 } else { t as u128 }
        }
    }

    /// ln(x) with x as NAD-scaled (>0). Returns NAD-scaled ln(x).
    /// Matches wadLn behavior via range reduction to [0.5, 2) and a tanh-series.
    #[inline]
    fn ln_nad(x_nad: u64) -> i128 {
        assert!(x_nad > 0, "ln_nad: x must be > 0");
        let mut z = x_nad as u128;
        let mut k: i128 = 0;

        // range reduce z into [NAD/2, 2*NAD)
        while z < (NAD as u128) / 2 {
            z = z.saturating_mul(2);
            k -= 1;
        }
        while z >= (NAD as u128) * 2 {
            z /= 2;
            k += 1;
        }

        // v = (z - NAD) / (z + NAD) in NAD (can be negative)
        let z_i = z as i128;
        let num = (z_i - NAD as i128) * NAD as i128; // (z-NAD)*NAD
        let den = (z_i + NAD as i128).max(1);        // (z+NAD)
        let v   = num / den;                         // NAD

        // ln(m) ≈ 2 * ( v + v^3/3 + v^5/5 + v^7/7 + v^9/9 )
        let v2  = (v * v) / (NAD as i128);
        let v3  = (v2 * v) / (NAD as i128);
        let v5  = (v3 * v2) / (NAD as i128);
        let v7  = (v5 * v2) / (NAD as i128);
        let v9  = (v7 * v2) / (NAD as i128);

        let series = v + v3 / 3 + v5 / 5 + v7 / 7 + v9 / 9;

        // ln(z) = 2*series + k*ln(2)
        let ln2 = NATURAL_LOG_OF_TWO_NAD as i128; // NAD
        2 * series + k * ln2
    }

    /// Convert basis points (bps) to NAD (1e9) fixed-point.
    #[inline]
    pub fn bps_to_nad(bps: u64) -> u64 {
        ((NAD as u128 * bps as u128) / (BPS_DENOMINATOR as u128)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a rate model with defaults (matches original behavior)
    fn default_rate_model() -> RateModel {
        RateModel::new(
            TARGET_UTIL_START_BPS,  // 50%
            TARGET_UTIL_END_BPS,    // 85%
            DEFAULT_RATE_HALF_LIFE_MS,  // 1 day
            DEFAULT_MIN_RATE_BPS,   // 1%
            0,                      // uncapped (default)
            DEFAULT_INITIAL_RATE_BPS,  // 2%
        )
    }

    // Helper to create original-style rate model for comparison
    // This simulates the OLD hardcoded behavior
    fn original_style_rate_model() -> RateModel {
        RateModel {
            exp_rate: NATURAL_LOG_OF_TWO_NAD / MS_PER_DAY,
            target_util_start: RateModel::bps_to_nad(TARGET_UTIL_START_BPS),
            target_util_end: RateModel::bps_to_nad(TARGET_UTIL_END_BPS),
            half_life_ms: MS_PER_DAY,
            min_rate: RateModel::bps_to_nad(100),  // OLD: hardcoded MIN_RATE_BPS = 100
            max_rate: 0,  // OLD: no max cap
            initial_rate: RateModel::bps_to_nad(200),  // OLD: hardcoded INITIAL_RATE_BPS = 200
        }
    }

    #[test]
    fn test_default_matches_original_high_util() {
        let default_model = default_rate_model();
        let original_model = original_style_rate_model();
        
        let last_rate = RateModel::bps_to_nad(200);  // 2%
        let time_elapsed = 3600_000;  // 1 hour
        let high_util = RateModel::bps_to_nad(9000);  // 90% > 85% target_end
        
        let (default_rate, default_integral) = default_model.calculate_rate(last_rate, time_elapsed, high_util);
        let (original_rate, original_integral) = original_model.calculate_rate(last_rate, time_elapsed, high_util);
        
        assert_eq!(default_rate, original_rate, "High util: rates should match");
        assert_eq!(default_integral, original_integral, "High util: integrals should match");
    }

    #[test]
    fn test_default_matches_original_low_util() {
        let default_model = default_rate_model();
        let original_model = original_style_rate_model();
        
        let last_rate = RateModel::bps_to_nad(500);  // 5%
        let time_elapsed = 3600_000;  // 1 hour
        let low_util = RateModel::bps_to_nad(3000);  // 30% < 50% target_start
        
        let (default_rate, default_integral) = default_model.calculate_rate(last_rate, time_elapsed, low_util);
        let (original_rate, original_integral) = original_model.calculate_rate(last_rate, time_elapsed, low_util);
        
        assert_eq!(default_rate, original_rate, "Low util: rates should match");
        assert_eq!(default_integral, original_integral, "Low util: integrals should match");
    }

    #[test]
    fn test_default_matches_original_middle_util() {
        let default_model = default_rate_model();
        let original_model = original_style_rate_model();
        
        let last_rate = RateModel::bps_to_nad(300);  // 3%
        let time_elapsed = 3600_000;  // 1 hour
        let mid_util = RateModel::bps_to_nad(7000);  // 70% - in optimal range
        
        let (default_rate, default_integral) = default_model.calculate_rate(last_rate, time_elapsed, mid_util);
        let (original_rate, original_integral) = original_model.calculate_rate(last_rate, time_elapsed, mid_util);
        
        assert_eq!(default_rate, original_rate, "Mid util: rates should match");
        assert_eq!(default_integral, original_integral, "Mid util: integrals should match");
    }

    #[test]
    fn test_uncapped_rate_grows_exponentially() {
        let model = default_rate_model();
        
        let last_rate = RateModel::bps_to_nad(200);  // 2%
        let time_elapsed = MS_PER_DAY;  // 1 day (one half-life)
        let high_util = RateModel::bps_to_nad(9000);  // 90%
        
        let (new_rate, _) = model.calculate_rate(last_rate, time_elapsed, high_util);
        
        // After 1 half-life of high util, rate should approximately double
        // Allow 5% tolerance for Taylor approximation
        let expected = last_rate * 2;
        let tolerance = expected / 20;
        assert!(
            new_rate > expected - tolerance && new_rate < expected + tolerance,
            "Rate should ~double after 1 half-life: got {}, expected ~{}", 
            new_rate, expected
        );
    }

    #[test]
    fn test_max_cap_enforced() {
        // Create model with max cap at 10%
        let model = RateModel::new(
            TARGET_UTIL_START_BPS,
            TARGET_UTIL_END_BPS,
            DEFAULT_RATE_HALF_LIFE_MS,
            DEFAULT_MIN_RATE_BPS,
            1000,  // 10% max cap
            DEFAULT_INITIAL_RATE_BPS,
        );
        
        let last_rate = RateModel::bps_to_nad(800);  // 8%
        let time_elapsed = MS_PER_DAY * 7;  // 7 days - would grow way past 10%
        let high_util = RateModel::bps_to_nad(9000);
        
        let (new_rate, _) = model.calculate_rate(last_rate, time_elapsed, high_util);
        let max_rate = RateModel::bps_to_nad(1000);
        
        assert_eq!(new_rate, max_rate, "Rate should be capped at max");
    }

    #[test]
    fn test_min_floor_enforced() {
        let model = default_rate_model();
        
        let last_rate = RateModel::bps_to_nad(150);  // 1.5%
        let time_elapsed = MS_PER_DAY * 30;  // 30 days of low util
        let low_util = RateModel::bps_to_nad(1000);  // 10% util
        
        let (new_rate, _) = model.calculate_rate(last_rate, time_elapsed, low_util);
        let min_rate = RateModel::bps_to_nad(DEFAULT_MIN_RATE_BPS);
        
        assert!(new_rate >= min_rate, "Rate should not go below min floor");
    }

    #[test]
    fn test_faster_half_life_adjusts_quicker() {
        // Fast model: 1 hour half-life
        let fast_model = RateModel::new(
            TARGET_UTIL_START_BPS,
            TARGET_UTIL_END_BPS,
            MIN_RATE_HALF_LIFE_MS,  // 1 hour
            DEFAULT_MIN_RATE_BPS,
            0,
            DEFAULT_INITIAL_RATE_BPS,
        );
        
        // Slow model: 7 day half-life
        let slow_model = RateModel::new(
            TARGET_UTIL_START_BPS,
            TARGET_UTIL_END_BPS,
            MAX_RATE_HALF_LIFE_MS,  // 7 days
            DEFAULT_MIN_RATE_BPS,
            0,
            DEFAULT_INITIAL_RATE_BPS,
        );
        
        let last_rate = RateModel::bps_to_nad(200);
        let time_elapsed = 3600_000;  // 1 hour
        let high_util = RateModel::bps_to_nad(9000);
        
        let (fast_rate, _) = fast_model.calculate_rate(last_rate, time_elapsed, high_util);
        let (slow_rate, _) = slow_model.calculate_rate(last_rate, time_elapsed, high_util);
        
        // Fast model should have higher rate after same time
        assert!(
            fast_rate > slow_rate,
            "Faster half-life should result in higher rate: fast={}, slow={}",
            fast_rate, slow_rate
        );
        
        // Fast model should approximately double (1 hour = 1 half-life)
        let expected_fast = last_rate * 2;
        let tolerance = expected_fast / 10;
        assert!(
            fast_rate > expected_fast - tolerance,
            "Fast model should ~double: got {}, expected ~{}", 
            fast_rate, expected_fast
        );
    }

    #[test]
    fn test_validation_rejects_invalid_params() {
        // Half-life too short
        assert!(!RateModel::validate_rate_params(
            MIN_RATE_HALF_LIFE_MS - 1,
            DEFAULT_MIN_RATE_BPS,
            0,
            DEFAULT_INITIAL_RATE_BPS
        ));
        
        // Half-life too long
        assert!(!RateModel::validate_rate_params(
            MAX_RATE_HALF_LIFE_MS + 1,
            DEFAULT_MIN_RATE_BPS,
            0,
            DEFAULT_INITIAL_RATE_BPS
        ));
        
        // min > max (when max is set)
        assert!(!RateModel::validate_rate_params(
            DEFAULT_RATE_HALF_LIFE_MS,
            500,   // 5% min
            200,   // 2% max - invalid!
            300
        ));
        
        // initial < min
        assert!(!RateModel::validate_rate_params(
            DEFAULT_RATE_HALF_LIFE_MS,
            500,   // 5% min
            0,
            200    // 2% initial - below min!
        ));
        
        // initial > max (when max is set)
        assert!(!RateModel::validate_rate_params(
            DEFAULT_RATE_HALF_LIFE_MS,
            100,
            500,   // 5% max
            1000   // 10% initial - above max!
        ));
    }

    #[test]
    fn test_validation_accepts_valid_params() {
        // Default params
        assert!(RateModel::validate_rate_params(
            DEFAULT_RATE_HALF_LIFE_MS,
            DEFAULT_MIN_RATE_BPS,
            0,  // uncapped
            DEFAULT_INITIAL_RATE_BPS
        ));
        
        // With max cap
        assert!(RateModel::validate_rate_params(
            DEFAULT_RATE_HALF_LIFE_MS,
            100,   // 1% min
            1000,  // 10% max
            500    // 5% initial
        ));
        
        // Extreme but valid
        assert!(RateModel::validate_rate_params(
            MIN_RATE_HALF_LIFE_MS,
            0,
            MAX_ALLOWED_RATE_BPS,
            MIN_INITIAL_RATE_BPS
        ));
    }
}