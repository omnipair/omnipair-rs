use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

// Re-export modules to simplify the program interface.
pub use state::*;
pub use utils::*;
pub use instructions::*;

declare_id!("GZqkUaCeaf96tm2Jw1QaY88fduMHnP7bhLTwjqDk6LM6");

#[program]
pub mod omnipair {
    use super::*;

    // Factory instructions
    pub fn initialize_factory(ctx: Context<InitializeFactory>) -> Result<()> {
        instructions::factory_initialize::initialize_factory(ctx)
    }

    pub fn create_pair(ctx: Context<CreatePair>, rate_model: Pubkey) -> Result<()> {
        instructions::factory_create_pair::create_pair(ctx, rate_model)
    }

    pub fn get_pairs(ctx: Context<GetPairs>) -> Result<Vec<Pubkey>> {
        instructions::factory_get_pairs::get_pairs(ctx)
    }

    // Rate model instructions
    pub fn create_rate_model(ctx: Context<CreateRateModel>) -> Result<()> {
        instructions::rate_model_create::create_rate_model(ctx)
    }

    // Pair instructions
    pub fn initialize_pair(ctx: Context<InitializePair>) -> Result<()> {
        instructions::pair_initialize::initialize_pair(ctx)
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        instructions::pair_swap::swap(ctx, amount_in, min_amount_out)
    }

    pub fn adjust_collateral(
        ctx: Context<AdjustCollateral>,
        amount0: i64,
        amount1: i64,
    ) -> Result<()> {
        instructions::pair_adjust_collateral::adjust_collateral(ctx, amount0, amount1)
    }

    pub fn adjust_debt(
        ctx: Context<AdjustDebt>,
        amount0: i64,
        amount1: i64,
    ) -> Result<()> {
        instructions::pair_adjust_debt::adjust_debt(ctx, amount0, amount1)
    }

    pub fn flashloan(
        ctx: Context<Flashloan>,
        amount0: u64,
        amount1: u64,
        data: Vec<u8>,
    ) -> Result<()> {
        instructions::pair_flashloan::flashloan(ctx, amount0, amount1, data)
    }

    pub fn withdraw_liquidation_bond(ctx: Context<WithdrawLiquidationBond>) -> Result<()> {
        instructions::pair_withdraw_liquidation_bond::withdraw_liquidation_bond(ctx)
    }
}
