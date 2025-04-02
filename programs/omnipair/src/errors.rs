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
