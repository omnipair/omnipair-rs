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

// EMA constants
#[constant]
pub const DEFAULT_HALF_LIFE: u64 = 7 * 60; // 7 minutes (recommended)
pub const MIN_HALF_LIFE: u64 = 1 * 60; // 1 minute
pub const MAX_HALF_LIFE: u64 = 12 * 60 * 60; // 12 hours
pub const DEPLOYER_MAX_FEE_BPS: u16 = 1000; // 10%
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180; // ln(2) scaled by NAD

// Pair constants
pub const MIN_LIQUIDITY: u64 = 1_000; // 10^3
pub const INITIAL_RATE_BPS: u64 = 200; // 2%
pub const MIN_RATE_BPS: u64 = 100;      // 1%

// Default IRM constants
pub const TARGET_UTIL_START_BPS: u64 = 3_300; // 33%
pub const TARGET_UTIL_END_BPS: u64 = 6_600; // 66%
pub const SECONDS_PER_DAY: u64 = 86_400;
pub const SECONDS_PER_YEAR: u64 = 31_536_000;

// Math constants
pub const MAX_X_E18: u128 = 161_324_830_204_992_680_279;
pub const MAX_X_E12: u64 = 289_279_112_968_179;

// Global Seeds for deterministic PDAs
#[constant]
pub const PAIR_SEED_PREFIX: &[u8] = b"gamm_pair";
#[constant]
pub const LP_MINT_SEED_PREFIX: &[u8] = b"gamm_lp_mint";
#[constant]
pub const FACTORY_SEED_PREFIX: &[u8] = b"gamm_factory";
#[constant]
pub const POSITION_SEED_PREFIX: &[u8] = b"gamm_position";
#[constant]
pub const PAIR_CONFIG_SEED_PREFIX: &[u8] = b"gamm_pair_config";
#[constant]
pub const FUTARCHY_AUTHORITY_SEED_PREFIX: &[u8] = b"futarchy_authority";