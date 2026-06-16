use anchor_lang::error;

use crate::constants::*;
use crate::errors::ErrorCode;
use crate::shared::math::ceil_div;

pub fn split_claim_minus_buffer(
    deposit_amount: u64,
    buffer_ratio_bps: u16,
) -> anchor_lang::prelude::Result<(u64, u64)> {
    anchor_lang::prelude::require!(
        buffer_ratio_bps > 0 && buffer_ratio_bps < BPS_DENOMINATOR,
        ErrorCode::InvalidMarketBufferRatio
    );

    let buffer_amount = ceil_div(
        (deposit_amount as u128)
            .checked_mul(buffer_ratio_bps as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflow)?;
    let buffer_amount = u64::try_from(buffer_amount).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let claim_amount = deposit_amount
        .checked_sub(buffer_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    Ok((claim_amount, buffer_amount))
}

pub fn required_buffer_for_claims(
    protected_claim_token_supply: u64,
    buffer_ratio_bps: u16,
) -> anchor_lang::prelude::Result<u64> {
    anchor_lang::prelude::require!(
        buffer_ratio_bps > 0 && buffer_ratio_bps < BPS_DENOMINATOR,
        ErrorCode::InvalidMarketBufferRatio
    );

    let claim_ratio_bps = BPS_DENOMINATOR
        .checked_sub(buffer_ratio_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let required_buffer = ceil_div(
        (protected_claim_token_supply as u128)
            .checked_mul(buffer_ratio_bps as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        claim_ratio_bps as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflow)?;

    u64::try_from(required_buffer).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub fn active_stake_units(
    staked_claim_token_amount: u64,
    staked_buffer_share_amount: u64,
    buffer_ratio_bps: u16,
) -> anchor_lang::prelude::Result<u64> {
    anchor_lang::prelude::require!(
        buffer_ratio_bps > 0 && buffer_ratio_bps < BPS_DENOMINATOR,
        ErrorCode::InvalidMarketBufferRatio
    );

    let claim_ratio_bps = BPS_DENOMINATOR
        .checked_sub(buffer_ratio_bps)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let claim_units = (staked_claim_token_amount as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(claim_ratio_bps as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let buffer_units = (staked_buffer_share_amount as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(buffer_ratio_bps as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;

    u64::try_from(claim_units.min(buffer_units)).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub fn accrue_fee_liability(
    active_stake_units: u64,
    current_fee_index_nad: u128,
    checkpoint_fee_index_nad: u128,
) -> anchor_lang::prelude::Result<u64> {
    let delta = current_fee_index_nad
        .checked_sub(checkpoint_fee_index_nad)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let accrued = (active_stake_units as u128)
        .checked_mul(delta)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;

    u64::try_from(accrued).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

pub fn require_market_reserve_floor(
    post_reserve: u64,
    protected_claim_token_supply: u64,
    required_buffer: u64,
) -> anchor_lang::prelude::Result<()> {
    let floor = protected_claim_token_supply
        .checked_add(required_buffer)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    anchor_lang::prelude::require_gte!(
        post_reserve,
        floor,
        ErrorCode::InsufficientMarketClaimCoverage
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn claim_minus_buffer_mints_only_protected_claim() {
        let (claim, buffer) = split_claim_minus_buffer(1_000_000, 2_000).unwrap();
        assert_eq!(claim, 800_000);
        assert_eq!(buffer, 200_000);
    }

    #[test]
    fn claim_minus_buffer_rounds_buffer_up_for_dust() {
        let (claim, buffer) = split_claim_minus_buffer(2, 2_000).unwrap();
        assert_eq!(claim, 1);
        assert_eq!(buffer, 1);
        assert_eq!(required_buffer_for_claims(claim, 2_000).unwrap(), 1);
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
        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::InsufficientMarketClaimCoverage)
        );
    }

    proptest! {
        #[test]
        fn claim_minus_buffer_preserves_deposit_principal(
            deposit_amount in 2_u64..1_000_000_000_000_u64,
            buffer_ratio_bps in 1_u16..BPS_DENOMINATOR,
        ) {
            let (claim_amount, buffer_amount) =
                split_claim_minus_buffer(deposit_amount, buffer_ratio_bps).unwrap();

            prop_assert_eq!(claim_amount + buffer_amount, deposit_amount);
            prop_assert!(buffer_amount > 0);

            let required_buffer =
                required_buffer_for_claims(claim_amount, buffer_ratio_bps).unwrap();
            prop_assert!(
                buffer_amount >= required_buffer,
                "buffer_amount={} required_buffer={} claim_amount={} ratio={}",
                buffer_amount,
                required_buffer,
                claim_amount,
                buffer_ratio_bps
            );
        }

        #[test]
        fn reserve_floor_is_exact_claim_plus_required_buffer(
            protected_claim_token_supply in 1_u64..1_000_000_000_000_u64,
            required_buffer in 0_u64..1_000_000_000_000_u64,
            excess_reserve in 0_u64..1_000_000_000_u64,
        ) {
            let floor = protected_claim_token_supply.checked_add(required_buffer).unwrap();
            require_market_reserve_floor(
                floor + excess_reserve,
                protected_claim_token_supply,
                required_buffer,
            ).unwrap();

            if floor > 0 {
                let err = require_market_reserve_floor(
                    floor - 1,
                    protected_claim_token_supply,
                    required_buffer,
                ).unwrap_err();
                prop_assert_eq!(
                    err,
                    anchor_lang::prelude::error!(ErrorCode::InsufficientMarketClaimCoverage)
                );
            }
        }

        #[test]
        fn active_stake_uses_the_less_covered_side(
            staked_claim_token_amount in 1_u64..1_000_000_000_u64,
            staked_buffer_share_amount in 1_u64..1_000_000_000_u64,
            buffer_ratio_bps in 1_u16..BPS_DENOMINATOR,
        ) {
            let active_units =
                active_stake_units(staked_claim_token_amount, staked_buffer_share_amount, buffer_ratio_bps)
                    .unwrap();
            let claim_ratio_bps = BPS_DENOMINATOR - buffer_ratio_bps;
            let claim_units = (staked_claim_token_amount as u128)
                .checked_mul(BPS_DENOMINATOR as u128)
                .unwrap()
                / claim_ratio_bps as u128;
            let buffer_units = (staked_buffer_share_amount as u128)
                .checked_mul(BPS_DENOMINATOR as u128)
                .unwrap()
                / buffer_ratio_bps as u128;

            prop_assert_eq!(active_units as u128, claim_units.min(buffer_units));
            prop_assert!(active_units as u128 <= claim_units);
            prop_assert!(active_units as u128 <= buffer_units);
        }
    }
}
