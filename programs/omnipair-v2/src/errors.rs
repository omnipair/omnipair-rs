use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid deployer")]
    InvalidDeployer,

    #[msg("Argument missing")]
    ArgumentMissing,

    #[msg("Invalid swap fee bps")]
    InvalidSwapFeeBps,

    #[msg("Invalid interest fee bps")]
    InvalidInterestFeeBps,

    #[msg("Invalid half life")]
    InvalidHalfLife,

    #[msg("Invalid futarchy authority")]
    InvalidFutarchyAuthority,

    #[msg("Invalid reduce-only authority")]
    InvalidReduceOnlyAuthority,

    #[msg("Invalid market manager")]
    InvalidMarketManager,

    #[msg("Invalid market config authority")]
    InvalidMarketConfigAuthority,

    #[msg("Market governance timelock is not ready")]
    GovernanceTimelockNotReady,

    #[msg("Invalid argument")]
    InvalidArgument,

    #[msg("Amount cannot be zero")]
    AmountZero,

    #[msg("Insufficient amount0 in")]
    InsufficientAmount0In,

    #[msg("Insufficient amount1 in")]
    InsufficientAmount1In,

    #[msg("Borrowing power exceeded")]
    BorrowingPowerExceeded,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Invalid token program")]
    InvalidTokenProgram,

    #[msg("Borrow exceeds reserve")]
    BorrowExceedsReserve,

    #[msg("Insufficient amount0")]
    InsufficientAmount0,

    #[msg("Insufficient amount1")]
    InsufficientAmount1,

    #[msg("Insufficient output amount")]
    InsufficientOutputAmount,

    #[msg("Output amount below minimum requested (slippage exceeded)")]
    SlippageExceeded,

    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,

    #[msg("Insufficient cash reserve0")]
    InsufficientCashReserve0,

    #[msg("Insufficient cash reserve1")]
    InsufficientCashReserve1,

    #[msg("Arithmetic overflow")]
    Overflow,

    #[msg("Undercollateralized")]
    Undercollateralized,

    #[msg("Insufficient balance for collateral")]
    InsufficientBalanceForCollateral,

    #[msg("Insufficient amount")]
    InsufficientAmount,

    #[msg("User balance insufficient to cover requested amount")]
    InsufficientBalance,

    #[msg("Insufficient debt")]
    InsufficientDebt,

    #[msg("User position not initialized")]
    UserPositionNotInitialized,

    #[msg("Zero debt amount")]
    ZeroDebtAmount,

    #[msg("Not undercollateralized")]
    NotUndercollateralized,

    #[msg("Broken invariant")]
    BrokenInvariant,

    #[msg("Math overflow during invariant calculation")]
    InvariantOverflow,

    #[msg("Math overflow during fee calculation.")]
    FeeMathOverflow,

    #[msg("Math overflow during output amount calculation.")]
    OutputAmountOverflow,

    #[msg("Math overflow during reserve calculation.")]
    ReserveOverflow,

    #[msg("Math underflow during reserve calculation.")]
    ReserveUnderflow,

    #[msg("Math underflow during cash reserve calculation.")]
    CashReserveUnderflow,

    #[msg("Math overflow during denominator calculation.")]
    DenominatorOverflow,

    #[msg("Math overflow during liquidity calculation")]
    LiquidityMathOverflow,

    #[msg("Math overflow during liquidity square root calculation")]
    LiquiditySqrtOverflow,

    #[msg("Math underflow during liquidity calculation")]
    LiquidityUnderflow,

    #[msg("Math overflow during liquidity conversion")]
    LiquidityConversionOverflow,

    #[msg("Math overflow during supply calculation")]
    SupplyOverflow,

    #[msg("Math underflow during supply calculation")]
    SupplyUnderflow,

    #[msg("Math overflow during debt calculation")]
    DebtMathOverflow,

    #[msg("Math overflow during debt share calculation")]
    DebtShareMathOverflow,

    #[msg("Math overflow during debt share division")]
    DebtShareDivisionOverflow,

    #[msg("Math overflow during debt utilization calculation")]
    DebtUtilizationOverflow,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Invalid mint length")]
    InvalidMintLen,

    #[msg("Invalid distribution - percentages must sum to 100%")]
    InvalidDistribution,

    #[msg("Invalid protocol auction config")]
    InvalidAuctionConfig,

    #[msg("Protocol auction reference price is stale")]
    StaleAuctionReference,

    #[msg("Protocol auction payment is insufficient")]
    InsufficientAuctionPayment,

    #[msg("Invalid LP mint key")]
    InvalidLpMintKey,

    #[msg("Invalid LP name")]
    InvalidLpName,

    #[msg("Invalid LP symbol")]
    InvalidLpSymbol,

    #[msg("Invalid LP URI")]
    InvalidLpUri,

    #[msg("Account not empty")]
    AccountNotEmpty,

    #[msg("Invalid mint authority")]
    InvalidMintAuthority,

    #[msg("Frozen LP mint")]
    FrozenLpMint,

    #[msg("Non-zero supply")]
    NonZeroSupply,

    #[msg("Wrong LP decimals")]
    WrongLpDecimals,

    #[msg("Invalid vault - token_in_vault and token_out_vault must be different")]
    InvalidVaultSameAccount,

    #[msg("Invalid vault")]
    InvalidVault,

    #[msg("Invalid params hash - hash does not match computed parameters")]
    InvalidParamsHash,

    #[msg("Invalid version")]
    InvalidVersion,

    #[msg("Invalid token order")]
    InvalidTokenOrder,

    #[msg("Invalid rate model - rate_model does not match market configuration")]
    InvalidRateModel,

    #[msg("Invalid position market - position does not match market")]
    InvalidPositionMarket,

    #[msg("Invalid utilization bounds - must satisfy: MIN <= start < end <= MAX")]
    InvalidUtilBounds,

    #[msg("Invalid rate parameters - check half_life_ms, min_rate_bps, max_rate_bps, initial_rate_bps bounds")]
    InvalidRateParams,

    #[msg("Operation blocked: reduce-only mode is active")]
    ReduceOnlyMode,

    #[msg("Cannot remove collateral in reduce-only mode while debt exists")]
    ReduceOnlyHasDebt,

    #[msg("Operation blocked: same-transaction liquidity delta detected")]
    LiquidityDeltaCircuitBreaker,

    #[msg("Operation blocked: liquidity delta instruction must be top-level")]
    LiquidityDeltaCircuitBreakerCpi,

    #[msg("Invalid instructions sysvar")]
    InvalidInstructionsSysvar,

    #[msg("Insufficient post-withdraw debt coverage")]
    InsufficientPostWithdrawDebtCoverage,

    #[msg("Invalid recipient - address does not match configured revenue recipient")]
    InvalidRecipient,

    #[msg("Invalid market")]
    InvalidMarket,

    #[msg("Invalid market config")]
    InvalidMarketConfig,

    #[msg("Invalid settlement price")]
    InvalidSettlementPrice,

    #[msg("Market reserve share backing is insufficient")]
    InsufficientMarketShareBacking,

    #[msg("Invalid market side")]
    InvalidMarketSide,

    #[msg("Invalid yield account")]
    InvalidYieldAccount,

    #[msg("Invalid hLP vault")]
    InvalidHlpVault,

    #[msg("Not enough remaining accounts")]
    NotEnoughAccounts,

    #[msg("hLP settlement is unavailable")]
    HlpSettlementUnavailable,

    #[msg("Borrow headroom is insufficient")]
    InsufficientBorrowHeadroom,

    #[msg("Market health is insufficient")]
    InsufficientMarketHealth,

    #[msg("Invalid margin position")]
    InvalidMarginPosition,

    #[msg("Recognized collateral is insufficient")]
    InsufficientRecognizedCollateral,

    #[msg("Position is not liquidatable")]
    PositionNotLiquidatable,

    #[msg("Insurance coverage is insufficient")]
    InsufficientInsurance,

    #[msg("Socialized liquidation loss exceeds caller cap")]
    LiquidationSocializationExceeded,

    #[msg("Claim mint must not charge transfer fees")]
    InvalidClaimMint,

    #[msg("Fee liability is not backed by fee vault balance")]
    UnbackedFeeLiability,

    #[msg("Invalid market fee authority")]
    InvalidMarketFeeAuthority,

    #[msg("Market is reduce-only")]
    MarketReduceOnly,

    #[msg("Market has not started")]
    MarketNotStarted,

    #[msg("Market math overflow")]
    MarketMathOverflow,

    #[msg("Daily liquidity limit exceeded")]
    DailyLimitExceeded,

    #[msg("Market risk circuit breaker triggered")]
    MarketRiskCircuitBreaker,

    #[msg("Instruction is intentionally not live yet")]
    InstructionNotLive,

    #[msg("Liquidation repay amount exceeds partial liquidation cap")]
    LiquidationRepayTooLarge,

    #[msg("Invalid liquidation auction")]
    InvalidLiquidationAuction,

    #[msg("Liquidation auction is already active")]
    LiquidationAuctionAlreadyActive,

    #[msg("Liquidation auction is inactive")]
    LiquidationAuctionInactive,

    #[msg("Liquidation auction is stale")]
    StaleLiquidationAuction,
}
