use anchor_lang::error;

use crate::constants::*;
use crate::errors::ErrorCode;

pub fn split_claim_minus_buffer(
    deposit_amount: u64,
    buffer_ratio_bps: u16,
) -> anchor_lang::prelude::Result<(u64, u64)> {
    anchor_lang::prelude::require!(
        buffer_ratio_bps > 0 && buffer_ratio_bps < BPS_DENOMINATOR,
        ErrorCode::InvalidMarketBufferRatioV2
    );

    let buffer_amount = (deposit_amount as u128)
        .checked_mul(buffer_ratio_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let buffer_amount = u64::try_from(buffer_amount).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
    let claim_amount = deposit_amount
        .checked_sub(buffer_amount)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;

    Ok((claim_amount, buffer_amount))
}

pub fn active_stake_units(
    staked_claim_amount: u64,
    staked_buffer_shares: u64,
    buffer_ratio_bps: u16,
) -> anchor_lang::prelude::Result<u64> {
    anchor_lang::prelude::require!(
        buffer_ratio_bps > 0 && buffer_ratio_bps < BPS_DENOMINATOR,
        ErrorCode::InvalidMarketBufferRatioV2
    );

    let claim_ratio_bps = BPS_DENOMINATOR
        .checked_sub(buffer_ratio_bps)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let claim_units = (staked_claim_amount as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(claim_ratio_bps as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let buffer_units = (staked_buffer_shares as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(buffer_ratio_bps as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;

    u64::try_from(claim_units.min(buffer_units)).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

pub fn accrue_fee_liability(
    active_stake_units: u64,
    current_fee_index_nad: u128,
    checkpoint_fee_index_nad: u128,
) -> anchor_lang::prelude::Result<u64> {
    let delta = current_fee_index_nad
        .checked_sub(checkpoint_fee_index_nad)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let accrued = (active_stake_units as u128)
        .checked_mul(delta)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;

    u64::try_from(accrued).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

pub fn require_market_reserve_floor(
    post_reserve: u64,
    protected_claim_supply: u64,
    required_buffer: u64,
) -> anchor_lang::prelude::Result<()> {
    let floor = protected_claim_supply
        .checked_add(required_buffer)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    anchor_lang::prelude::require_gte!(
        post_reserve,
        floor,
        ErrorCode::InsufficientMarketClaimCoverageV2
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_minus_buffer_mints_only_protected_claim() {
        let (claim, buffer) = split_claim_minus_buffer(1_000_000, 2_000).unwrap();
        assert_eq!(claim, 800_000);
        assert_eq!(buffer, 200_000);
    }

    #[test]
    fn active_stake_requires_matched_claim_and_buffer() {
        let full = active_stake_units(800_000, 200_000, 2_000).unwrap();
        let claim_short = active_stake_units(400_000, 200_000, 2_000).unwrap();
        let buffer_short = active_stake_units(800_000, 100_000, 2_000).unwrap();

        assert_eq!(full, 1_000_000);
        assert_eq!(claim_short, 500_000);
        assert_eq!(buffer_short, 500_000);
    }

    #[test]
    fn fees_are_index_based_and_non_compounding() {
        let fees = accrue_fee_liability(1_000_000, 3 * NAD as u128, NAD as u128).unwrap();
        assert_eq!(fees, 2_000_000);
    }

    #[test]
    fn reserve_floor_protects_claims_and_required_buffer() {
        require_market_reserve_floor(1_150, 1_000, 150).unwrap();
        let err = require_market_reserve_floor(1_149, 1_000, 150).unwrap_err();
        assert_eq!(err, anchor_lang::prelude::error!(ErrorCode::InsufficientMarketClaimCoverageV2));
    }
}
