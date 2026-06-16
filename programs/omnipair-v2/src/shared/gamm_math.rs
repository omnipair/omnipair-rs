use crate::errors::ErrorCode;
use crate::shared::math::ceil_div;
use anchor_lang::prelude::*;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_amount_out_matches_constant_product_rounding_down() {
        let amount_out = CPCurve::calculate_amount_out(1_000, 2_000, 100).unwrap();

        assert_eq!(amount_out, 181);
    }

    #[test]
    fn calculate_amount_in_rounds_up_to_cover_requested_output() {
        let amount_in = CPCurve::calculate_amount_in(1_000, 2_000, 181).unwrap();

        assert_eq!(amount_in, 100);
        assert!(CPCurve::calculate_amount_out(1_000, 2_000, amount_in).unwrap() >= 181);
    }
}
