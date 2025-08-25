use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::math::*;

#[account]
pub struct RateModel {
    /// exp_rate: NAD/second (k_real = exp_rate / NAD)
    pub exp_rate: u64,
    /// utilization band edges (NAD-scaled: 0..NAD)
    pub target_util_start: u64,
    pub target_util_end: u64,
}

impl RateModel {
    pub fn new() -> Self {
        const SECONDS_PER_HOUR: u64 = 3_600;
        Self {
            // For production you likely want ln(2)/day; for testing we use 1 hour.
            // exp_rate: NATURAL_LOG_OF_TWO_NAD / SECONDS_PER_DAY,
            exp_rate: NATURAL_LOG_OF_TWO_NAD / SECONDS_PER_HOUR,
            target_util_start: Self::bps_to_nad(TARGET_UTIL_START_BPS),
            target_util_end:   Self::bps_to_nad(TARGET_UTIL_END_BPS),
        }
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
        let exp_rate = self.exp_rate as u128;                     // NAD/sec
        let x        = exp_rate.saturating_mul(dt);               // NAD (k*dt in NAD units)
        let gd       = taylor_exp(-(x as i64), NAD, TAYLOR_TERMS) as u128; // NAD ≈ e^{-(x/NAD)}

        let min_nad  = Self::bps_to_nad(MIN_RATE_BPS) as u128;

        // Enforce only the MIN floor; no MAX ceiling.
        let last = (last_rate as u128).max(min_nad);

        // High util: exponential growth (no cap)
        if (last_util as u128) > (self.target_util_end as u128) {
            // r1 = r0 * e^{+k dt} = r0 * NAD / gd
            let curr = last
                .saturating_mul(NAD as u128)
                / gd.max(1);

            // ∫ r dt = (r1 - r0) / k_real = (r1 - r0) * NAD / exp_rate, then / YEAR
            let numer    = curr.saturating_sub(last).saturating_mul(NAD as u128);
            let integral = numer / exp_rate / (SECONDS_PER_YEAR as u128);
            return (curr as u64, integral as u64);
        }

        // Low util: exponential decay with MIN floor
        if (last_util as u128) < (self.target_util_start as u128) {
            // r1 = r0 * e^{-k dt} = r0 * gd / NAD
            let r1_unclamped = last.saturating_mul(gd) / (NAD as u128);

            if r1_unclamped >= min_nad {
                let curr = r1_unclamped;
                // ∫ = (r0 - r1) * NAD / exp_rate, then / YEAR
                let numer    = last.saturating_sub(curr).saturating_mul(NAD as u128);
                let integral = numer / exp_rate / (SECONDS_PER_YEAR as u128);
                return (curr as u64, integral as u64);
            } else {
                // Hit MIN during window → split: exponential down to MIN, then flat MIN
                if last <= min_nad {
                    let integral = min_nad.saturating_mul(dt) / (SECONDS_PER_YEAR as u128);
                    return (min_nad as u64, integral as u64);
                }
                let t_to_min = Self::time_to_reach_closed_form(last, min_nad, exp_rate, /*up=*/false)
                    .min(dt);

                // exp part (to floor): (last - MIN) * NAD / exp_rate
                let exp_part  = last.saturating_sub(min_nad).saturating_mul(NAD as u128) / exp_rate;
                // flat tail: MIN * (dt - t*)
                let flat_part = min_nad.saturating_mul(dt.saturating_sub(t_to_min));
                let integral  = (exp_part + flat_part) / (SECONDS_PER_YEAR as u128);
                return (min_nad as u64, integral as u64);
            }
        }

        // Middle band: flat
        let integral = (last.saturating_mul(dt)) / (SECONDS_PER_YEAR as u128);
        (last as u64, integral as u64)
    }

    /// Closed-form time to reach target using ln.
    /// up=false : r(t) = r0 * e^{-k t} <= target  ⇒  t = ln(r0/target) * NAD / exp_rate
    #[inline]
    fn time_to_reach_closed_form(r0: u128, target: u128, exp_rate_nad_per_s: u128, up: bool) -> u128 {
        if up {
            // Not used now (no max cap), but kept for parity.
            if target <= r0 { return 0; }
            let ratio_nad = (target.saturating_mul(NAD as u128)) / r0.max(1);
            let ln_ratio  = Self::ln_nad(ratio_nad as u64);  // NAD (signed)
            let t = ((NAD as i128) * ln_ratio) / (exp_rate_nad_per_s as i128);
            if t <= 0 { 0 } else { t as u128 }
        } else {
            if r0 <= target { return 0; }
            let ratio_nad = (r0.saturating_mul(NAD as u128)) / target.max(1);
            let ln_ratio  = Self::ln_nad(ratio_nad as u64);  // NAD (signed)
            let t = ((NAD as i128) * ln_ratio) / (exp_rate_nad_per_s as i128);
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