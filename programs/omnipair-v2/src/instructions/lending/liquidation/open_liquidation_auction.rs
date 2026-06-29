use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{LiquidationAuctionOpened, MarketEventMetadata},
    shared::account::get_size_with_discriminator,
    state::{
        liquidation_auction_reference_price_nad, liquidation_auction_start_incentive_bps,
        LiquidationAuction, MarginPosition, Market,
    },
};

use crate::instructions::common::require_supported_asset_mint;

#[derive(Accounts)]
pub struct OpenLiquidationAuction<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,
    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [
            MARGIN_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.owner.as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPosition>>,

    #[account(
        init_if_needed,
        payer = keeper,
        space = get_size_with_discriminator::<LiquidationAuction>(),
        seeds = [
            LIQUIDATION_AUCTION_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.key().as_ref(),
            debt_asset_mint.key().as_ref(),
        ],
        bump
    )]
    pub liquidation_auction: Box<Account<'info, LiquidationAuction>>,

    pub system_program: Program<'info, System>,
}

impl<'info> OpenLiquidationAuction<'info> {
    pub fn validate(&self) -> Result<()> {
        self.market.assert_started()?;
        let debt_asset = self.market.asset_for_mint(self.debt_asset_mint.key())?;
        require_keys_eq!(
            self.market.side(debt_asset.opposite())?.asset_mint,
            self.collateral_asset_mint.key(),
            ErrorCode::InvalidMint
        );
        require_supported_asset_mint(&self.debt_asset_mint)?;
        require_supported_asset_mint(&self.collateral_asset_mint)?;
        require_keys_eq!(
            self.margin_position.market,
            self.market.key(),
            ErrorCode::InvalidMarginPosition
        );
        require!(
            self.liquidation_auction.can_open_for(&self.margin_position),
            ErrorCode::LiquidationAuctionAlreadyActive
        );
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self) -> Result<()> {
        self.update()?;
        self.validate()
    }

    pub fn handle_open(ctx: Context<Self>) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let margin_position_key = ctx.accounts.margin_position.key();
        let borrower_key = ctx.accounts.margin_position.owner;
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let collateral_asset_mint_key = ctx.accounts.collateral_asset_mint.key();
        let current_slot = Clock::get()?.slot;
        let debt_asset = ctx.accounts.market.asset_for_mint(debt_asset_mint_key)?;

        require!(
            ctx.accounts
                .liquidation_auction
                .can_open_for(&ctx.accounts.margin_position),
            ErrorCode::LiquidationAuctionAlreadyActive
        );
        let health_bps = ctx
            .accounts
            .market
            .position_health_bps(&ctx.accounts.margin_position, debt_asset)?;
        require!(
            health_bps < ctx.accounts.market.config.market_health_min_bps as u64,
            ErrorCode::PositionNotLiquidatable
        );
        let terms = ctx
            .accounts
            .market
            .liquidation_terms(&ctx.accounts.margin_position, debt_asset)?;
        let start_incentive_bps = liquidation_auction_start_incentive_bps(
            ctx.accounts
                .market
                .config
                .liquidation_auction_start_incentive_bps,
            terms.liquidation_incentive_bps,
        )?;
        let reference_price_nad =
            liquidation_auction_reference_price_nad(&ctx.accounts.market, debt_asset)?;

        ctx.accounts
            .liquidation_auction
            .open(crate::state::OpenLiquidationAuctionParams {
                market: market_key,
                margin_position: margin_position_key,
                borrower: borrower_key,
                debt_asset,
                debt_mint: debt_asset_mint_key,
                collateral_mint: collateral_asset_mint_key,
                position_risk_epoch: ctx.accounts.margin_position.risk_epoch,
                current_slot,
                duration_slots: ctx
                    .accounts
                    .market
                    .config
                    .liquidation_auction_duration_slots,
                start_health_bps: health_bps,
                start_incentive_bps,
                max_incentive_bps: terms.liquidation_incentive_bps,
                max_repay_amount: terms.max_repay_amount,
                reference_price_nad,
                bump: ctx.bumps.liquidation_auction,
            })?;

        emit!(LiquidationAuctionOpened {
            market: market_key,
            margin_position: margin_position_key,
            borrower: borrower_key,
            debt_asset_mint: debt_asset_mint_key,
            collateral_asset_mint: collateral_asset_mint_key,
            start_slot: current_slot,
            end_slot: ctx.accounts.liquidation_auction.end_slot,
            start_health_bps: health_bps,
            start_incentive_bps,
            max_incentive_bps: terms.liquidation_incentive_bps,
            max_repay_amount: terms.max_repay_amount,
            reference_price_nad,
            metadata: MarketEventMetadata::new(ctx.accounts.keeper.key(), market_key)?,
        });
        Ok(())
    }
}
