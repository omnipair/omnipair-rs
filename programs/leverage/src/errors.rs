use anchor_lang::prelude::*;

#[error_code]
pub enum LeverageError {
    #[msg("Amount must be greater than zero")]
    AmountZero,
    #[msg("Multiplier must be > 1x (multiplier_bps > 10_000)")]
    MultiplierTooLow,
    #[msg("max_slippage_bps must be <= 10_000")]
    InvalidSlippage,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Pair has no liquidity")]
    InsufficientLiquidity,
    #[msg("Failed to decode internal callback data")]
    InvalidCallbackData,
    #[msg("Swap returned zero tokens")]
    SwapFailed,
    #[msg("No open debt found — position may already be closed or never opened")]
    PositionNotOpen,
    #[msg("Expected 12 remaining_accounts (pair..user_leverage_position)")]
    MissingRemainingAccounts,
    #[msg("remaining_accounts key does not match the corresponding validated account")]
    RemainingAccountMismatch,
}
