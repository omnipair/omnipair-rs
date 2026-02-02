use anchor_lang::prelude::*;

// GLOBAL CONSTANTS
/// NAD: Nine-decimal fixed point unit (1e9 scaling), similar to WAD (1e18) by Maker.
#[constant]
pub const NAD: u64 = 1_000_000_000;
#[constant]
pub const NAD_DECIMALS: u8 = 9;
#[constant]
pub const BPS_DENOMINATOR: u16 = 10_000;
#[constant]
pub const CLOSE_FACTOR_BPS: u16 = 5_000; // 50%
#[constant]
pub const MAX_COLLATERAL_FACTOR_BPS: u16 = 8_500; // 85% cap for dynamic collateral factor
#[constant]
pub const LTV_BUFFER_BPS: u16 = 500; // 5% buffer between borrow limit and liquidation threshold
#[constant]
pub const FLASHLOAN_FEE_BPS: u16 = 5; // 0.05%
#[constant]
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 50; // 0.5% liquidation incentive for caller
#[constant]
pub const LIQUIDATION_PENALTY_BPS: u16 = 300; // 3% total liquidation penalty (0.5% to liquidator, 2.5% to LPs)
#[constant]
pub const LIQUIDITY_WITHDRAWAL_FEE_BPS: u16 = 100; // 1% fee on liquidity withdrawal (goes to remaining LPs)
#[constant]
pub const PAIR_CREATION_FEE_LAMPORTS: u64 = 200_000_000; // 0.2 SOL
// 3log2(100) = 19.93 secs (with 400ms slot time, this is ~50 slots)
#[constant]
pub const DIRECTIONAL_EMA_HALF_LIFE_MS: u64 = 3_000; // 3 seconds
/// The nominal slot duration in milliseconds.
#[constant]
pub const TARGET_MS_PER_SLOT: u64 = 400;


// EMA constants (in milliseconds)
pub const MIN_HALF_LIFE_MS: u64 = 1 * 60 * 1_000; // 1 minute
pub const MAX_HALF_LIFE_MS: u64 = 12 * 60 * 60 * 1_000; // 12 hours
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180; // ln(2) scaled by NAD

// Pair constants
pub const MIN_LIQUIDITY: u64 = 1_000; // 10^3

// Rate model configurable bounds
pub const DEFAULT_INITIAL_RATE_BPS: u64 = 200;  // 2% default starting rate
pub const DEFAULT_MIN_RATE_BPS: u64 = 100;      // 1% default floor
pub const DEFAULT_MAX_RATE_BPS: u64 = 0;        // 0 = uncapped by default (no ceiling)

// Rate bounds validation limits
pub const MIN_ALLOWED_RATE_BPS: u64 = 0;        // Pools can set floor to 0%
pub const MAX_ALLOWED_RATE_BPS: u64 = 100_000;  // Pools can set ceiling up to 1000%
pub const MIN_INITIAL_RATE_BPS: u64 = 10;       // Initial rate must be at least 0.1%
pub const MAX_INITIAL_RATE_BPS: u64 = 10_000;   // Initial rate cannot exceed 100%

// Rate half-life bounds (controls adjustment speed)
pub const DEFAULT_RATE_HALF_LIFE_MS: u64 = MS_PER_DAY;  // 1 day default (current behavior)
pub const MIN_RATE_HALF_LIFE_MS: u64 = 1 * 60 * 60 * 1_000;  // 1 hour minimum (fastest adjustment)
pub const MAX_RATE_HALF_LIFE_MS: u64 = 30 * 24 * 60 * 60 * 1_000;  // 30 days maximum (slowest adjustment)

/// Debt share scaling factor for increased precision floor in rounded division.
pub const DEBT_SHARE_SCALE: u64 = 1_000_000; // 10^6

// Default IRM constants
pub const TARGET_UTIL_START_BPS: u64 = 5_000; // 50%
pub const TARGET_UTIL_END_BPS: u64 = 8_500; // 85%
pub const MILLISECONDS_PER_YEAR: u64 = 31_536_000_000; // 31,536,000 seconds * 1000

// Rate model constants
pub const MS_PER_DAY: u64 = 86_400_000;
// Utilization bounds (configurable per pool within these limits)
pub const MIN_TARGET_UTIL_BPS: u64 = 100;  // 1% minimum for target_util_start
pub const MAX_TARGET_UTIL_BPS: u64 = 10_000;  // 100% maximum for target_util_end

// Global Seeds for deterministic PDAs
#[constant]
pub const PAIR_SEED_PREFIX: &[u8] = b"gamm_pair";
#[constant]
pub const POSITION_SEED_PREFIX: &[u8] = b"gamm_position";
#[constant]
pub const FUTARCHY_AUTHORITY_SEED_PREFIX: &[u8] = b"futarchy_authority";
#[constant]
pub const METADATA_SEED_PREFIX: &[u8] = b"metadata";
#[constant]
pub const RESERVE_VAULT_SEED_PREFIX: &[u8] = b"reserve_vault";
#[constant]
pub const COLLATERAL_VAULT_SEED_PREFIX: &[u8] = b"collateral_vault";
#[constant]
pub const VERSION: u8 = 1;