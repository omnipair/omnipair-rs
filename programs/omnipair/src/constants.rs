// GLOBAL CONSTANTS
/// NAD: Nine-decimal fixed point unit (1e9 scaling), similar to WAD (1e18) by Maker.
pub const NAD: u64 = 1_000_000_000;
pub const NAD_DECIMALS: u8 = 9;
pub const BPS_DENOMINATOR: u64 = 10_000;

// EMA constants
pub const DEFAULT_HALF_LIFE: u64 = 24 * 60 * 60; // 24 hours
pub const TAYLOR_TERMS: u64 = 5;
pub const NATURAL_LOG_OF_TWO_NAD: u64 = 693_147_180; // ln(2) scaled by NAD

// Pair constants
pub const MIN_LIQUIDITY: u64 = 1_000; // 10^3
pub const FEE_BPS: u64 = 30;
// Hardcoded for this PoC, will be dynamic in production
// TODO: Should be dynamic based on an internal liquidity oracle
// to avoid profiting from utilizing fixed collateral factor when < swap slippage (for cheaper swaps)
// see: https://docs.omnipair.fi/technical-breakdown/liquidity-oracle-and-dynamic-collateral-factor
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
pub const PAIR_SEED_PREFIX: &[u8] = b"gamm_pair";
pub const LP_MINT_SEED_PREFIX: &[u8] = b"gamm_lp_mint";
pub const FACTORY_SEED_PREFIX: &[u8] = b"gamm_factory";
pub const TOKEN_VAULT_SEED_PREFIX: &[u8] = b"gamm_token_vault";
pub const POSITION_SEED_PREFIX: &[u8] = b"gamm_position";
