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
pub use instructions::faucet_mint::FaucetMint;

declare_id!("FaAt1g93kZWVZbEqLe7McjtHu5Ev4i2mWRJb85fKyEpZ");

#[program]
pub mod omnipair {
    use super::*;

    /// View instructions for client data access (Logs + RPC simulation to parse returned logs for values)
    /// This approach allows for "view" functionality of on-chain calculations (similar to Solidity view functions)
    /// i.e. time-dependent calculations
    pub fn view_pair_data(ctx: Context<ViewPairData>, getter: PairViewKind) -> Result<()> {
        ViewPairData::handle_view_data(ctx, getter)
    }

    pub fn view_user_position_data(ctx: Context<ViewUserPositionData>, getter: UserPositionViewKind) -> Result<()> {
        ViewUserPositionData::handle_view_data(ctx, getter)
    }

    // Pair instructions
    #[access_control(ctx.accounts.validate_and_create_rate_model())]
    pub fn initialize_pair(ctx: Context<InitializePair>) -> Result<()> {
        InitializePair::handle_initialize(ctx)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn bootstrap_pair(ctx: Context<BootstrapPair>, args: AddLiquidityArgs) -> Result<()> {
        BootstrapPair::handle_bootstrap(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_add(&args))]
    pub fn add_liquidity(
        ctx: Context<AdjustLiquidity>,
        args: AddLiquidityArgs,
    ) -> Result<()> {
        AdjustLiquidity::handle_add(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_remove(&args))]
    pub fn remove_liquidity(
        ctx: Context<AdjustLiquidity>,
        args: RemoveLiquidityArgs,
    ) -> Result<()> {
        AdjustLiquidity::handle_remove(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_swap(&args))]
    pub fn swap(
        ctx: Context<Swap>,
        args: SwapArgs,
    ) -> Result<()> {
        Swap::handle_swap(ctx, args)
    }

    // Lending instructions
    #[access_control(ctx.accounts.update_and_validate_add(&args))]
    pub fn add_collateral(ctx: Context<AddCollateral>, args: AdjustPositionArgs) -> Result<()> {
        AddCollateral::handle_add_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_remove(&args))]
    pub fn remove_collateral(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_remove_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_borrow(&args))]
    pub fn borrow(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_repay(&args))]
    pub fn repay(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_repay(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_liquidate())]
    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        Liquidate::handle_liquidate(ctx)
    }

    // Faucet instruction
    #[cfg(feature = "development")]
    pub fn faucet_mint(ctx: Context<FaucetMint>) -> Result<()> {
        FaucetMint::handle_faucet_mint(ctx)
    }
}
