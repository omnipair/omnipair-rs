use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Insufficient input amount.")]
    InsufficientInputAmount,
    #[msg("Insufficient liquidity.")]
    InsufficientLiquidity,
    #[msg("Expired deadline.")]
    Expired,
    #[msg("Flashloan not repaid.")]
    FlashloanNotRepaid,
    #[msg("Invalid token order. token0 must be less than token1.")]
    InvalidTokenOrder,
    #[msg("Factory is full. Cannot create more pairs.")]
    FactoryFull,
}

#[error_code]
pub enum ErrorCode {
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
    
    #[msg("Invalid rent")]
    InvalidRent,
    
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

    #[msg("Factory is full")]
    FactoryFull,

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

    #[msg("Flashloan not repaid")]
    FlashloanNotRepaid,

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
}
