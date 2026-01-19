use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid deployer")]
    InvalidDeployer,

    #[msg("Argument missing")]
    ArgumentMissing,

    #[msg("Invalid swap fee bps")]
    InvalidSwapFeeBps,

    #[msg("Invalid half life")]
    InvalidHalfLife,

    #[msg("Invalid futarchy authority")]
    InvalidFutarchyAuthority,

    #[msg("Invalid argument")]
    InvalidArgument,
    
    #[msg("Insufficient collateral")]
    InsufficientCollateral,

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

    #[msg("Math overflow during cash reserve calculation.")]
    CashReserveOverflow,

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
    
    #[msg("Invalid vault - token_in_vault must be owned by the pair")]
    InvalidVaultIn,
    
    #[msg("Invalid vault - token_out_vault must be owned by the pair")]
    InvalidVaultOut,
    
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

    #[msg("Invalid utilization bounds - must satisfy: MIN <= start < end <= MAX")]
    InvalidUtilBounds,
}
