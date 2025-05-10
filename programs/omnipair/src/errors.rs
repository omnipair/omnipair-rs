use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid argument")]
    InvalidArgument,

    #[msg("Pair already initialized")]
    PairAlreadyInitialized,

    #[msg("Invalid K value")]
    InvalidK,
    
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
    
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Invalid rate model")]
    InvalidRateModel,
    
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    
    #[msg("Invalid token program")]
    InvalidTokenProgram,
    
    #[msg("Invalid system program")]
    InvalidSystemProgram,
    
    #[msg("Invalid signer")]
    InvalidSigner,
    
    #[msg("Invalid account")]
    InvalidAccount,
    
    #[msg("Invalid program")]
    InvalidProgram,
    
    #[msg("Invalid instruction")]
    InvalidInstruction,
    
    #[msg("Invalid state")]
    InvalidState,
    
    #[msg("Invalid calculation")]
    InvalidCalculation,
    
    #[msg("Invalid time")]
    InvalidTime,
    
    #[msg("Invalid price")]
    InvalidPrice,
    
    #[msg("Invalid rate")]
    InvalidRate,
    
    #[msg("Invalid utilization")]
    InvalidUtilization,

    #[msg("Invalid token order")]
    InvalidTokenOrder,

    #[msg("Debt not zero")]
    DebtNotZero,

    #[msg("Insufficient amount0")]
    InsufficientAmount0,
    
    #[msg("Insufficient amount1")]
    InsufficientAmount1,

    #[msg("Insufficient output amount")]
    InsufficientOutputAmount,

    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,

    #[msg("Arithmetic overflow")]
    Overflow,

    #[msg("Undercollateralized")]
    Undercollateralized,

    #[msg("Insufficient collateral for token0")]
    InsufficientCollateral0,

    #[msg("Insufficient collateral for token1")]
    InsufficientCollateral1,

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

    #[msg("Math overflow during liquidity division")]
    LiquidityDivisionOverflow,

    #[msg("Math overflow during supply calculation")]
    SupplyOverflow,

    #[msg("Math overflow during debt calculation")]
    DebtMathOverflow,

    #[msg("Math overflow during debt share calculation")]
    DebtShareMathOverflow,

    #[msg("Math overflow during debt share division")]
    DebtShareDivisionOverflow,

    #[msg("Math overflow during debt utilization calculation")]
    DebtUtilizationOverflow,
}
