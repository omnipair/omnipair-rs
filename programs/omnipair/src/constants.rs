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
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 300; // 3% liquidation incentive for caller
#[constant]
pub const PAIR_CREATION_FEE_LAMPORTS: u64 = 200_000_000; // 0.2 SOL


// EMA constants
pub const MIN_HALF_LIFE: u64 = 1 * 60; // 1 minute
pub const MAX_HALF_LIFE: u64 = 12 * 60 * 60; // 12 hours
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180; // ln(2) scaled by NAD

// Pair constants
pub const MIN_LIQUIDITY: u64 = 1_000; // 10^3
pub const INITIAL_RATE_BPS: u64 = 200; // 2%
pub const MIN_RATE_BPS: u64 = 100;      // 1%

// Default IRM constants
pub const TARGET_UTIL_START_BPS: u64 = 5_000; // 50%
pub const TARGET_UTIL_END_BPS: u64 = 8_500; // 85%
pub const SECONDS_PER_YEAR: u64 = 31_536_000;

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