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

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Omnipair",
    project_url: "https://omnipair.fi",
    contacts: "email:security@omnipair.fi,telegram:rustfully",
    encryption: "
    -----BEGIN PGP PUBLIC KEY BLOCK-----
    Comment: User ID:	elrakabawi <muhammed@omnipair.fi>
    Comment: Fingerprint:	211A 6E45 DF9C FED4 D274  C2AE 022F C8B7 FB82 0E26


    mDMEaXYfpBYJKwYBBAHaRw8BAQdAuJGsO1bf97ftK3AXLBGoGMVNsKfYEgbgFbTL
    XM61dt20IWVscmFrYWJhd2kgPG11aGFtbWVkQG9tbmlwYWlyLmZpPoivBBMWCgBX
    GxSAAAAAAAQADm1hbnUyLDIuNSsxLjExLDIsMQIbAwULCQgHAgIiAgYVCgkICwIE
    FgIDAQIeBwIXgBYhBCEabkXfnP7U0nTCrgIvyLf7gg4mBQJpdihJAAoJEAIvyLf7
    gg4mxfYBAPCkQftSqGfV5sxCRDNgWrgbwv0MIFN/PVVUMIvkJ/gFAQC6/sYGZrPK
    ebn6YVuRXB8fdUZdhN0jP/0NM0WPl350B7g4BGl2H6QSCisGAQQBl1UBBQEBB0B9
    TA7UtvyyduFFA9XzGdoI6+kX9//N0T8IdFAwYAPMSwMBCAeIlAQYFgoAPBsUgAAA
    AAAEAA5tYW51MiwyLjUrMS4xMSwyLDECGwwWIQQhGm5F35z+1NJ0wq4CL8i3+4IO
    JgUCaXYoTwAKCRACL8i3+4IOJsXTAQC0gR5fZXblXez9LuJGWTQgZGhbm7a/jquS
    DsC4cr6QOAD/eCbtxLgkh0XOvmCfdzeYezEAKATL+7g1Nyq2lPSmKQM=
    =pIZY
    -----END PGP PUBLIC KEY BLOCK-----
    ",
    source_code: "https://github.com/omnipair/omnipair-rs",
    source_release: env!("GIT_RELEASE"),
    source_revision: env!("GIT_REV"),
    auditors: "Offside Labs, Ackee",
    policy: "https://omnipair.fi/security",
    acknowledgements: "Hurricane @ Offside Labs & Juan (github.com/0xjuaan) @ Obsidian Audits"
}

declare_id!("Bd9Uhf5S8yzfop8cG9oqRs6jVcLtu8B4cb2gvRmtbNzk");

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

    pub fn update_futarchy_authority(ctx: Context<UpdateFutarchyAuthority>, args: UpdateFutarchyAuthorityArgs) -> Result<()> {
        UpdateFutarchyAuthority::handle_update(ctx, args)
    }

    pub fn update_protocol_revenue(ctx: Context<UpdateProtocolRevenue>, args: UpdateProtocolRevenueArgs) -> Result<()> {
        UpdateProtocolRevenue::handle_update(ctx, args)
    }

    pub fn distribute_tokens(ctx: Context<DistributeTokens>, args: DistributeTokensArgs) -> Result<()> {
        DistributeTokens::handle_distribute(ctx, args)
    }

    #[access_control(ctx.accounts.update())]
    pub fn claim_protocol_fees(ctx: Context<ClaimProtocolFees>) -> Result<()> {
        ClaimProtocolFees::handle_claim(ctx)
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
    pub fn add_collateral(ctx: Context<AddCollateral>, args: AdjustCollateralArgs) -> Result<()> {
        AddCollateral::handle_add_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_remove(&args))]
    pub fn remove_collateral(ctx: Context<CommonAdjustCollateral>, args: AdjustCollateralArgs) -> Result<()> {
        CommonAdjustCollateral::handle_remove_collateral(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_borrow(&args))]
    pub fn borrow(ctx: Context<CommonAdjustDebt>, args: AdjustDebtArgs) -> Result<()> {
        CommonAdjustDebt::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate_repay(&args))]
    pub fn repay(ctx: Context<CommonAdjustDebt>, args: AdjustDebtArgs) -> Result<()> {
        CommonAdjustDebt::handle_repay(ctx, args)
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
