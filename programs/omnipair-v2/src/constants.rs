use anchor_lang::{prelude::*, solana_program::pubkey};

/// NAD: nine-decimal fixed point unit, similar to WAD in EVM systems.
#[constant]
pub const NAD: u64 = 1_000_000_000;
#[constant]
pub const NAD_DECIMALS: u8 = 9;
#[constant]
pub const BPS_DENOMINATOR: u16 = 10_000;
pub(crate) const MAX_COLLATERAL_FACTOR_BPS: u16 = 8_500; // 85% cap for dynamic collateral factor
pub(crate) const LTV_BUFFER_BPS: u16 = 500; // 5% buffer between borrow limit and liquidation threshold
#[constant]
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 50;
#[constant]
pub const TARGET_MS_PER_SLOT: u64 = 400;

pub const MIN_HALF_LIFE_MS: u64 = 60_000;
pub const MAX_HALF_LIFE_MS: u64 = 12 * 60 * 60 * 1_000;
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180;
pub const MS_PER_DAY: u64 = 86_400_000;
pub const MIN_LIQUIDITY: u64 = 1_000;

#[constant]
pub const MARKET_V2_SEED_PREFIX: &[u8] = b"market_v2";
#[constant]
pub const MARKET_RESERVE_VAULT_SEED_PREFIX: &[u8] = b"market_reserve";
#[constant]
pub const MARKET_COLLATERAL_VAULT_SEED_PREFIX: &[u8] = b"market_collateral";
#[constant]
pub const MARKET_FEE_VAULT_SEED_PREFIX: &[u8] = b"market_fee";
#[constant]
pub const MARKET_STAKE_VAULT_SEED_PREFIX: &[u8] = b"market_stake";
#[constant]
pub const STAKE_POSITION_SEED_PREFIX: &[u8] = b"stake";
#[constant]
pub const MARGIN_POSITION_SEED_PREFIX: &[u8] = b"margin";
#[constant]
pub const HEDGE_VAULT_SEED_PREFIX: &[u8] = b"hedged";
#[constant]
pub const HEDGE_POSITION_SEED_PREFIX: &[u8] = b"hedge_position";
#[constant]
pub const INSURANCE_RESERVE_SEED_PREFIX: &[u8] = b"insurance";
#[constant]
pub const MARKET_VERSION: u8 = 2;

/// Emergency signer authorized to toggle reduce-only mode.
pub const REDUCE_ONLY_EMERGENCY_AUTHORITY: Pubkey =
    pubkey!("3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV");
