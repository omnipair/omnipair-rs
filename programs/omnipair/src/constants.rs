pub const SCALE: u64 = 1_000_000;
pub const PRICE_PRECISION: u64 = 1_000_000;
pub const DEFAULT_HALF_LIFE: u64 = 24 * 60 * 60; // 24 hours
pub const TAYLOR_TERMS: u64 = 5;
pub const SCALED_NATURAL_LOG_OF_TWO: u64 = 693_147_180; // ln(2) scaled by 1e9

// Pair constants
pub const MINIMUM_LIQUIDITY: u64 = 1_000; // 10^3
pub const FEE_BPS: u64 = 30;
pub const CF_BPS: u64 = 8_500;
pub const MIN_RATE: u64 = 1; // 0.0001%
pub const MAX_RATE: u64 = 1000000; // 100%

// Rate Model constants
pub const TARGET_UTIL_START: u64 = 330_000_000; // 33% (0.33e18 in Solidity)
pub const TARGET_UTIL_END: u64 = 660_000_000; // 66% (0.66e18 in Solidity)
pub const SECONDS_PER_DAY: u64 = 86_400;
pub const SECONDS_PER_YEAR: u64 = 31_536_000;

// Math constants
pub const MAX_X_E18: u128 = 161_324_830_204_992_680_279;
pub const MAX_X_E12: u64 = 289_279_112_968_179;

// Global Seeds for deterministic PDAs
pub const GAMM_LP_MINT_SEED_PREFIX: &[u8] = b"gamm_lp_mint";
pub const GAMM_PAIR_SEED_PREFIX: &[u8] = b"gamm_pair";
pub const GAMM_FACTORY_SEED_PREFIX: &[u8] = b"gamm_factory";
pub const GAMM_RESERVE_VAULT_SEED_PREFIX: &[u8] = b"gamm_reserve_vault";
pub const GAMM_COLLATERAL_VAULT_SEED_PREFIX: &[u8] = b"gamm_collateral_vault";

