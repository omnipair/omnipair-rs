use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod shared;
pub mod state;
pub mod tokens;
pub mod transitions;
pub mod utils;

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

declare_id!("oMNi2XGwWxDbEvhS2pWRQ6dtw8GkNBV42hfLZD6WmMF");

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

    #[access_control(ctx.accounts.validate())]
    pub fn claim_protocol_fees(ctx: Context<ClaimProtocolFees>) -> Result<()> {
        ClaimProtocolFees::handle_claim(ctx)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn initialize(ctx: Context<InitializeMarket>, args: InitializeMarketArgs) -> Result<()> {
        InitializeMarket::handle_initialize(ctx, args)
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

    #[access_control(ctx.accounts.validate(&args))]
    pub fn add_liquidity(ctx: Context<AddLiquidity>, args: AddLiquidityArgs) -> Result<()> {
        AddLiquidity::handle_add_liquidity(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
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

    #[access_control(ctx.accounts.validate(&args))]
    pub fn claim_yield(ctx: Context<ClaimYield>, args: ClaimYieldArgs) -> Result<()> {
        ClaimYield::handle_claim(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn swap<'info>(ctx: Context<'_, '_, '_, 'info, Swap<'info>>, args: SwapArgs) -> Result<()> {
        Swap::handle_swap(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn deposit_collateral(
        ctx: Context<DepositCollateral>,
        args: DepositCollateralArgs,
    ) -> Result<()> {
        DepositCollateral::handle_deposit(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn withdraw_collateral(
        ctx: Context<WithdrawCollateral>,
        args: WithdrawCollateralArgs,
    ) -> Result<()> {
        WithdrawCollateral::handle_withdraw(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn borrow(ctx: Context<Borrow>, args: BorrowArgs) -> Result<()> {
        Borrow::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn repay(ctx: Context<Repay>, args: RepayArgs) -> Result<()> {
        Repay::handle_repay(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn liquidate(ctx: Context<Liquidate>, args: LiquidateArgs) -> Result<()> {
        Liquidate::handle_liquidate(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn open_hedge(ctx: Context<OpenHedge>, args: OpenHedgeArgs) -> Result<()> {
        OpenHedge::handle_open(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn close_hedge(ctx: Context<CloseHedge>, args: CloseHedgeArgs) -> Result<()> {
        CloseHedge::handle_close(ctx, args)
    }

    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        crate::instructions::transfer_hook::handle_transfer_hook(program_id, accounts, data)
    }
}
