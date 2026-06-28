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
pub const MAX_MANAGER_FEE_BPS: u16 = 500;
#[constant]
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 100;
#[constant]
pub const LIQUIDATION_MAX_INCENTIVE_BPS: u16 = 500;
#[constant]
pub const LIQUIDATION_INSURANCE_FUNDING_BPS: u16 = 200;
#[constant]
pub const LIQUIDATION_PENALTY_BPS: u16 = 300;
#[constant]
pub const MARKET_CREATION_FEE_LAMPORTS: u64 = 200_000_000; // 0.2 SOL, same fee as V1 pair creation
#[constant]
pub const TARGET_MS_PER_SLOT: u64 = 400;
#[constant]
pub const MARKET_GOVERNANCE_DELAY_SLOTS: u64 = 216_000; // ~24 hours at 400ms/slot

pub const MIN_HALF_LIFE_MS: u64 = 60_000;
pub const MAX_HALF_LIFE_MS: u64 = 12 * 60 * 60 * 1_000;
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180;
pub const MS_PER_DAY: u64 = 86_400_000;
pub const MS_PER_YEAR: u64 = 365 * MS_PER_DAY;
pub const MIN_LIQUIDITY: u64 = 1_000;

// ADAPTIVE-CURVE INTEREST RATE MODEL
// A fixed-shape curve anchored at the target utilization, multiplied by a
// per-market `rate_at_target` that drifts toward the target over time:
//
//   instantaneous_rate(u) = rate_at_target * curve(error(u))
//   error(u) in [-1, 1], 0 at target; curve in [1/steepness, steepness]
//   rate_at_target_next = rate_at_target * e^(speed * error * dt/year)  (clamped)
//
// The curve gives an immediate, graded response to utilization; the anchor
// makes the *level* market-driven (no hardcoded ceiling), so the protocol
// never has to know the "right" rate in advance.
/// Target utilization the controller steers toward (bps of supplied liquidity).
pub const INTEREST_TARGET_UTILIZATION_BPS: u64 = 9_000; // 90%
/// Curve multiplier at full utilization (and its reciprocal at 0%), NAD-scaled.
/// 4x means the instantaneous rate ranges [rate_at_target/4, rate_at_target*4].
pub const INTEREST_CURVE_STEEPNESS_NAD: u128 = (NAD as u128) * 4;
/// Controller speed: e-folding rate per year of `rate_at_target` at full error.
/// Tuned gentle (level ~doubles in ~2 weeks at full error) since the curve
/// already provides the fast response.
pub const INTEREST_ADJUSTMENT_SPEED_PER_YEAR: u128 = 20;
/// Lower/upper bounds and initial value for the adaptive anchor (APR in NAD).
pub const INTEREST_MIN_RATE_AT_TARGET_NAD: u128 = (NAD as u128) / 1_000; // 0.1% APR
pub const INTEREST_MAX_RATE_AT_TARGET_NAD: u128 = (NAD as u128) * 2; // 200% APR
pub const INTEREST_INITIAL_RATE_AT_TARGET_NAD: u128 = (NAD as u128) * 4 / 100; // 4% APR
/// Cap on the per-accrual exponent (NAD), bounding the anchor's move in a single
/// step so a stale market can't jump violently (clamped further by min/max).
pub const INTEREST_MAX_ADAPTATION_STEP_NAD: i128 = (NAD as i128) / 2;
/// Upper bound on the elapsed time charged in a single accrual, to bound
/// index growth (and therefore overflow / abuse) for very stale markets.
pub const MAX_INTEREST_ACCRUAL_MS: u64 = MS_PER_YEAR;

// HEDGED-LP PRE/POST TRACKING SOLVER (Phase 2)
// Compile-time gate for the swap-time pre-adjustment solve. Kept `false` until
// the hot-path orchestration has CU profiling and off-chain/on-chain quote
// parity validated on a validator; when `false` the swap path is unchanged.
pub const HLP_PRE_SOLVE_ENABLED: bool = false;
/// Only run the (expensive) pre/post solve when the estimated within-swap
/// tracking loss exceeds this NAD threshold; below it the cheap post-swap
/// rebalance is sufficient.
pub const HLP_PRE_SOLVE_LOSS_THRESHOLD_NAD: u128 = NAD as u128;
/// Fixed bisection iteration budget for the pre-adjustment solve (bounded so
/// the on-chain solve has a deterministic, CU-bounded cost).
pub const HLP_PRE_SOLVE_MAX_ITERS: u32 = 24;

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
pub const METADATA_SEED_PREFIX: &[u8] = b"metadata";
#[constant]
pub const INSURANCE_SEED_PREFIX: &[u8] = b"insurance";
#[constant]
pub const LIQUIDATION_AUCTION_SEED_PREFIX: &[u8] = b"liquidation_auction";
#[constant]
pub const MARKET_VERSION: u8 = 2;

/// Emergency signer authorized to toggle reduce-only mode.
pub const REDUCE_ONLY_EMERGENCY_AUTHORITY: Pubkey =
    pubkey!("3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV");
