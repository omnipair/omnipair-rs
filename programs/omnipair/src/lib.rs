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
pub use instructions::pair_initialize::InitializePair;

declare_id!("6yrECZ3GNeKfKhyAq9Ne8hJu4gUhVUmaB9pXneKosnPb");

#[program]
pub mod omnipair {
    use super::*;

    // Rate model instructions
    pub fn create_rate_model(ctx: Context<CreateRateModel>) -> Result<()> {
        CreateRateModel::handle_create(ctx)
    }

    // Pair instructions
    #[access_control(ctx.accounts.validate())]
    pub fn initialize_pair(ctx: Context<InitializePair>) -> Result<()> {
        InitializePair::handle_initialize(ctx)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn bootstrap_pair(ctx: Context<BootstrapPair>, args: AddLiquidityArgs) -> Result<()> {
        BootstrapPair::handle_bootstrap(ctx, args)
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
