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
pub use instructions::emit_value::{EmitValueArgs, PairViewKind, UserPositionViewKind, ViewPairData, ViewUserPositionData};

declare_id!("2h6oKUk4jcNQ81EzKvYVtzsyRpJwsz6J2pEeQTo1KsQB");

pub mod deployer {
    use super::{pubkey, Pubkey};
    
    #[cfg(feature = "development")]
    pub const ID: Pubkey = pubkey!("C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds");
    
    #[cfg(feature = "production")]
    pub const ID: Pubkey = pubkey!("8tF4uYMBXqGhCUGRZL3AmPqRzbX8JJ1TpYnY3uJKN4kt");
    
    // Default to development if no feature is specified
    #[cfg(not(any(feature = "development", feature = "production")))]
    pub const ID: Pubkey = pubkey!("C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds");
}

#[program]
pub mod omnipair {
    use super::*;

    /// View instructions for client data access (Logs + RPC simulation to parse returned logs for values)
    /// This approach allows for "view" functionality of on-chain calculations (similar to Solidity view functions)
    /// i.e. time-dependent calculations
    pub fn view_pair_data(ctx: Context<ViewPairData>, getter: PairViewKind, args: EmitValueArgs) -> Result<()> {
        ViewPairData::handle_view_data(ctx, getter, args)
    }

    pub fn view_user_position_data(ctx: Context<ViewUserPositionData>, getter: UserPositionViewKind) -> Result<()> {
        ViewUserPositionData::handle_view_data(ctx, getter)
    }

    // Futarchy authority instructions
    pub fn init_futarchy_authority(ctx: Context<InitFutarchyAuthority>, args: InitFutarchyAuthorityArgs) -> Result<()> {
        InitFutarchyAuthority::handle_init(ctx, args)
    }

    pub fn init_pair_config(ctx: Context<InitPairConfig>, args: InitPairConfigArgs) -> Result<()> {
        InitPairConfig::handle_init(ctx, args)
    }

    pub fn distribute_tokens(ctx: Context<DistributeTokens>, args: DistributeTokensArgs) -> Result<()> {
        DistributeTokens::handle_distribute(ctx, args)
    }

    // Pair instructions
    #[access_control(ctx.accounts.validate(&args))]
    pub fn initialize(ctx: Context<InitializeAndBootstrap>, args: InitializeAndBootstrapArgs) -> Result<()> {
        InitializeAndBootstrap::handle_initialize(ctx, args)
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

    // Flash loan instruction
    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn flashloan<'info>(ctx: Context<'_, '_, '_, 'info, Flashloan<'info>>, args: FlashloanArgs) -> Result<()> {
        Flashloan::handle_flashloan(ctx, args)
    }
}
