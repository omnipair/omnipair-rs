use anchor_lang::{prelude::*, solana_program::pubkey};

// GLOBAL CONSTANTS
/// NAD: Nine-decimal fixed point unit (1e9 scaling), similar to WAD (1e18) by Maker.
#[constant]
pub const NAD: u64 = 1_000_000_000;
#[constant]
pub const NAD_DECIMALS: u8 = 9;
#[constant]
pub const BPS_DENOMINATOR: u16 = 10_000;
#[constant]
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 100;
#[constant]
pub const LIQUIDATION_PENALTY_BPS: u16 = 300;
#[constant]
pub const MARKET_CREATION_FEE_LAMPORTS: u64 = 200_000_000; // 0.2 SOL, same fee as V1 pair creation
#[constant]
pub const TARGET_MS_PER_SLOT: u64 = 400;

pub const MIN_HALF_LIFE_MS: u64 = 60_000;
pub const MAX_HALF_LIFE_MS: u64 = 12 * 60 * 60 * 1_000;
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180;
pub const MS_PER_DAY: u64 = 86_400_000;
pub const MS_PER_YEAR: u64 = 365 * MS_PER_DAY;
pub const MIN_LIQUIDITY: u64 = 1_000;

// INTEREST RATE MODEL (kinked utilization curve, fixed APR in bps)
// rate(u) = base + slope1 * min(u, u*)/u*           for u <= u*
//         = base + slope1 + slope2 * (u - u*)/(1-u*) for u >  u*
/// Borrow APR at 0% utilization.
pub const INTEREST_BASE_RATE_BPS: u64 = 0;
/// Additional APR accrued linearly between 0% and optimal utilization.
pub const INTEREST_SLOPE1_BPS: u64 = 1_000; // +10% APR at the kink
/// Utilization kink (optimal point), in bps of total supplied liquidity.
pub const INTEREST_OPTIMAL_UTILIZATION_BPS: u64 = 8_000; // 80%
/// Steep APR slope applied between optimal and full utilization.
pub const INTEREST_SLOPE2_BPS: u64 = 30_000; // +300% APR from kink to 100%
/// Upper bound on the elapsed time charged in a single accrual, to bound
/// index growth (and therefore overflow / abuse) for very stale markets.
pub const MAX_INTEREST_ACCRUAL_MS: u64 = MS_PER_YEAR;

#[constant]
pub const MARKET_V2_SEED_PREFIX: &[u8] = b"market_v2";
#[constant]
pub const FUTARCHY_AUTHORITY_SEED_PREFIX: &[u8] = b"futarchy_authority";
#[constant]
pub const MARKET_RESERVE_VAULT_SEED_PREFIX: &[u8] = b"market_reserve";
#[constant]
pub const MARKET_COLLATERAL_VAULT_SEED_PREFIX: &[u8] = b"market_collateral";
#[constant]
pub const MARKET_FEE_VAULT_SEED_PREFIX: &[u8] = b"market_fee";
#[constant]
pub const MARKET_INTEREST_VAULT_SEED_PREFIX: &[u8] = b"market_interest";
#[constant]
pub const MARGIN_POSITION_SEED_PREFIX: &[u8] = b"margin";
#[constant]
pub const YIELD_ACCOUNT_SEED_PREFIX: &[u8] = b"yield";
#[constant]
pub const HLP_YLP_VAULT_SEED_PREFIX: &[u8] = b"hlp_ylp_vault";
#[constant]
pub const INSURANCE_SEED_PREFIX: &[u8] = b"insurance";
#[constant]
pub const MARKET_VERSION: u8 = 2;

/// Emergency signer authorized to toggle reduce-only mode.
pub const REDUCE_ONLY_EMERGENCY_AUTHORITY: Pubkey =
    pubkey!("3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV");
