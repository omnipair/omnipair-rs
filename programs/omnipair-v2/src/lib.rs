use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod shared;
pub mod state;
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
    auditors: "Offside Labs, Ackee",
    policy: "https://omnipair.fi/security"
}

declare_id!("358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv");

#[program]
pub mod omnipair_v2 {
    use super::*;

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

    pub fn set_reduce_only(
        ctx: Context<SetMarketReduceOnly>,
        args: SetMarketReduceOnlyArgs,
    ) -> Result<()> {
        SetMarketReduceOnly::handle_set(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn add_liquidity(ctx: Context<DepositReserve>, args: DepositReserveArgs) -> Result<()> {
        DepositReserve::handle_deposit(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn remove_liquidity(ctx: Context<RedeemClaim>, args: RedeemClaimArgs) -> Result<()> {
        RedeemClaim::handle_redeem(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn stake(ctx: Context<Stake>, args: StakeArgs) -> Result<()> {
        Stake::handle_stake(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn unstake(ctx: Context<Unstake>, args: UnstakeArgs) -> Result<()> {
        Unstake::handle_unstake(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn claim_fees(ctx: Context<ClaimFees>, args: ClaimFeesArgs) -> Result<()> {
        ClaimFees::handle_claim(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn claim_market_fees(
        ctx: Context<ClaimMarketFees>,
        args: ClaimMarketFeesArgs,
    ) -> Result<()> {
        ClaimMarketFees::handle_claim(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn swap(ctx: Context<MarketSwap>, args: MarketSwapArgs) -> Result<()> {
        MarketSwap::handle_swap(ctx, args)
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
    pub fn borrow(ctx: Context<MarketBorrow>, args: MarketBorrowArgs) -> Result<()> {
        MarketBorrow::handle_borrow(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn repay(ctx: Context<MarketRepay>, args: MarketRepayArgs) -> Result<()> {
        MarketRepay::handle_repay(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn deposit_insurance(
        ctx: Context<DepositInsurance>,
        args: DepositInsuranceArgs,
    ) -> Result<()> {
        DepositInsurance::handle_deposit(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn liquidate(ctx: Context<MarketLiquidate>, args: MarketLiquidateArgs) -> Result<()> {
        MarketLiquidate::handle_liquidate(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn open_hedge(ctx: Context<OpenHedge>, args: OpenHedgeArgs) -> Result<()> {
        OpenHedge::handle_open(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn close_hedge(ctx: Context<CloseHedge>, args: CloseHedgeArgs) -> Result<()> {
        CloseHedge::handle_close(ctx, args)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn claim_hedge_fees(ctx: Context<ClaimHedgeFees>, args: ClaimHedgeFeesArgs) -> Result<()> {
        ClaimHedgeFees::handle_claim(ctx, args)
    }
}
