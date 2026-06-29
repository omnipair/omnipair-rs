use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod shared;
pub mod state;

pub use instructions::*;
pub use state::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Omnipair V2",
    project_url: "https://omnipair.fi",
    contacts: "email:security@omnipair.fi,telegram:rustfully",
    source_code: "https://github.com/omnipair/omnipair-rs",
    source_release: env!("GIT_RELEASE"),
    source_revision: env!("GIT_REV"),
    auditors: "Pending final V2 security review",
    policy: "https://omnipair.fi/security"
}

declare_id!("358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv");

#[program]
pub mod omnipair_v2 {
    use super::*;

    pub fn init_futarchy_authority(
        ctx: Context<InitFutarchyAuthority>,
        args: InitFutarchyAuthorityArgs,
    ) -> Result<()> {
        InitFutarchyAuthority::handle_init(ctx, args)
    }

    pub fn update_futarchy_authority(
        ctx: Context<UpdateFutarchyAuthority>,
        args: UpdateFutarchyAuthorityArgs,
    ) -> Result<()> {
        UpdateFutarchyAuthority::handle_update(ctx, args)
    }

    pub fn update_protocol_revenue(
        ctx: Context<UpdateProtocolRevenue>,
        args: UpdateProtocolRevenueArgs,
    ) -> Result<()> {
        UpdateProtocolRevenue::handle_update(ctx, args)
    }

    pub fn update_revenue_recipients(
        ctx: Context<UpdateRevenueRecipients>,
        args: UpdateRevenueRecipientsArgs,
    ) -> Result<()> {
        UpdateRevenueRecipients::handle_update(ctx, args)
    }

    pub fn update_protocol_auction_config(
        ctx: Context<UpdateProtocolAuctionConfig>,
        args: UpdateProtocolAuctionConfigArgs,
    ) -> Result<()> {
        UpdateProtocolAuctionConfig::handle_update(ctx, args)
    }

    pub fn update_protocol_auction_recipients(
        ctx: Context<UpdateProtocolAuctionRecipients>,
        args: UpdateProtocolAuctionRecipientsArgs,
    ) -> Result<()> {
        UpdateProtocolAuctionRecipients::handle_update(ctx, args)
    }

    pub fn set_global_reduce_only(
        ctx: Context<SetGlobalReduceOnly>,
        args: SetGlobalReduceOnlyArgs,
    ) -> Result<()> {
        SetGlobalReduceOnly::handle_set_global_reduce_only(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn settle_protocol_auction<'info>(
        ctx: Context<'_, '_, '_, 'info, SettleProtocolAuction<'info>>,
        args: SettleProtocolAuctionArgs,
    ) -> Result<()> {
        SettleProtocolAuction::handle_settle(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn initialize(ctx: Context<InitializeMarket>, args: InitializeMarketArgs) -> Result<()> {
        InitializeMarket::handle_initialize(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn initialize_lp_metadata(
        ctx: Context<InitializeLpMetadata>,
        args: InitializeLpMetadataArgs,
    ) -> Result<()> {
        InitializeLpMetadata::handle_initialize(ctx, args)
    }

    pub fn update_config(
        ctx: Context<UpdateMarketConfig>,
        args: UpdateMarketConfigArgs,
    ) -> Result<()> {
        UpdateMarketConfig::handle_update(ctx, args)
    }

    #[access_control(ctx.accounts.validate())]
    pub fn set_reduce_only(
        ctx: Context<SetMarketReduceOnly>,
        args: SetMarketReduceOnlyArgs,
    ) -> Result<()> {
        SetMarketReduceOnly::handle_set(ctx, args)
    }

    pub fn set_operator(ctx: Context<SetMarketAuthority>, args: SetOperatorArgs) -> Result<()> {
        SetMarketAuthority::handle_set_operator(ctx, args)
    }

    pub fn set_manager(ctx: Context<SetMarketAuthority>, args: SetManagerArgs) -> Result<()> {
        SetMarketAuthority::handle_set_manager(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate())]
    pub fn claim_manager_fees(ctx: Context<ClaimManagerFees>) -> Result<()> {
        ClaimManagerFees::handle_claim(ctx)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn add_liquidity(ctx: Context<AddLiquidity>, args: AddLiquidityArgs) -> Result<()> {
        AddLiquidity::handle_add_liquidity(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        args: RemoveLiquidityArgs,
    ) -> Result<()> {
        RemoveLiquidity::handle_remove_liquidity(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn set_yield_recipient(
        ctx: Context<SetYieldRecipient>,
        args: SetYieldRecipientArgs,
    ) -> Result<()> {
        SetYieldRecipient::handle_set(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn claim_yield(ctx: Context<ClaimYield>, args: ClaimYieldArgs) -> Result<()> {
        ClaimYield::handle_claim(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn swap<'info>(ctx: Context<'_, '_, '_, 'info, Swap<'info>>, args: SwapArgs) -> Result<()> {
        Swap::handle_swap(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn deposit_collateral(
        ctx: Context<DepositCollateral>,
        args: DepositCollateralArgs,
    ) -> Result<()> {
        DepositCollateral::handle_deposit(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn withdraw_collateral(
        ctx: Context<WithdrawCollateral>,
        args: WithdrawCollateralArgs,
    ) -> Result<()> {
        WithdrawCollateral::handle_withdraw(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn borrow(ctx: Context<Borrow>, args: BorrowArgs) -> Result<()> {
        Borrow::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn repay(ctx: Context<Repay>, args: RepayArgs) -> Result<()> {
        Repay::handle_repay(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate())]
    pub fn open_liquidation_auction(ctx: Context<OpenLiquidationAuction>) -> Result<()> {
        OpenLiquidationAuction::handle_open(ctx)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn settle_liquidation_auction(
        ctx: Context<SettleLiquidationAuction>,
        args: SettleLiquidationAuctionArgs,
    ) -> Result<()> {
        SettleLiquidationAuction::handle_settle(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn deposit_single_sided(
        ctx: Context<DepositSingleSided>,
        args: DepositSingleSidedArgs,
    ) -> Result<()> {
        DepositSingleSided::handle_deposit(ctx, args)
    }

    #[access_control(ctx.accounts.update_and_validate(&args))]
    pub fn withdraw_single_sided(
        ctx: Context<WithdrawSingleSided>,
        args: WithdrawSingleSidedArgs,
    ) -> Result<()> {
        WithdrawSingleSided::handle_withdraw(ctx, args)
    }

    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        crate::instructions::transfer_hook::handle_transfer_hook(program_id, accounts, data)
    }
}
