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

declare_id!("FmSEiqY3RVgJp3Lyw7pbUFDHEbm8rZPqnrVc1TskBjBK");

#[program]
pub mod omnipair {
    use super::*;

    /// Emitters for front-end simulated getters (Emit + RPC simulation + Logs parsing)
    pub fn emit_pair_getters(ctx: Context<EmitPairValue>, getter: PairGetterType) -> Result<()> {
        EmitPairValue::handle_emit_value(ctx, getter)
    }

    pub fn emit_user_position_getters(ctx: Context<EmitUserPositionValue>, getter: UserPositionGetterType) -> Result<()> {
        EmitUserPositionValue::handle_emit_value(ctx, getter)
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

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        Swap::handle_swap(ctx, amount_in, min_amount_out)
    }

    // Lending instructions
    #[access_control(ctx.accounts.validate_add_and_update(&args))]
    pub fn add_collateral(ctx: Context<AddCollateral>, args: AdjustPositionArgs) -> Result<()> {
        AddCollateral::handle_add_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.validate_remove_and_update(&args))]
    pub fn remove_collateral(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_remove_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.validate_borrow_and_update(&args))]
    pub fn borrow(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.validate_repay_and_update(&args))]
    pub fn repay(ctx: Context<CommonAdjustPosition>, args: AdjustPositionArgs) -> Result<()> {
        CommonAdjustPosition::handle_repay(ctx, args)
    }

    // Faucet instruction
    #[cfg(feature = "development")]
    pub fn faucet_mint(ctx: Context<FaucetMint>) -> Result<()> {
        FaucetMint::handle_faucet_mint(ctx)
    }
}
