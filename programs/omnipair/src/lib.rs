use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

pub use utils::*;
pub use instructions::*;
pub use utils::account::*;

declare_id!("GZqkUaCeaf96tm2Jw1QaY88fduMHnP7bhLTwjqDk6LM6");

#[program]
pub mod omnipair {
    use super::*;

    // Rate model instructions
    pub fn create_rate_model(ctx: Context<CreateRateModel>) -> Result<()> {
        CreateRateModel::handle_create(ctx)
    }

    // Pair instructions
    #[access_control(ctx.accounts.validate(&args))]
    pub fn initialize_pair(ctx: Context<InitializePair>, args: AddLiquidityArgs) -> Result<()> {
        InitializePair::handle_initialize(ctx, args)
    }

    #[access_control(ctx.accounts.validate_add_and_update(&args))]
    pub fn add_liquidity(
        ctx: Context<AdjustLiquidity>,
        args: AddLiquidityArgs,
    ) -> Result<()> {
        AdjustLiquidity::handle_add(ctx, args)
    }

    #[access_control(ctx.accounts.validate_remove_and_update(&args))]
    pub fn remove_liquidity(
        ctx: Context<AdjustLiquidity>,
        args: RemoveLiquidityArgs,
    ) -> Result<()> {
        AdjustLiquidity::handle_remove(ctx, args)
    }

    // pub fn swap(
    //     ctx: Context<Swap>,
    //     amount_in: u64,
    //     min_amount_out: u64,
    // ) -> Result<()> {
    //     instructions::pair_swap::swap(ctx, amount_in, min_amount_out)
    // }
}
