use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{LiquidationAuctionSettled, MarketEventMetadata},
    generate_market_seeds,
    shared::token::{
        get_transfer_fee, transfer_from_user_to_vault, transfer_from_vault_to_user,
        transfer_from_vault_to_vault,
    },
    state::{FutarchyAuthority, LiquidationAuction, LiquidationPricing, MarginPosition, Market},
};

use super::common::validate_liquidation_accounts;
use crate::instructions::common::{
    require_supported_asset_mint, token_program_for_mint, validate_interest_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SettleLiquidationAuctionArgs {
    pub repay_amount: u64,
    pub min_collateral_out: u64,
    pub max_insurance_draw: u64,
    pub max_socialized_loss: u64,
}

#[derive(Accounts)]
#[instruction(args: SettleLiquidationAuctionArgs)]
pub struct SettleLiquidationAuction<'info> {
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

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    #[account(mut)]
    pub bidder: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,
    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub interest_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub collateral_insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub bidder_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub bidder_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.owner.as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPosition>>,

    #[account(
        mut,
        seeds = [
            LIQUIDATION_AUCTION_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.key().as_ref(),
            debt_asset_mint.key().as_ref(),
        ],
        bump = liquidation_auction.bump
    )]
    pub liquidation_auction: Box<Account<'info, LiquidationAuction>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> SettleLiquidationAuction<'info> {
    pub fn validate(&self, args: &SettleLiquidationAuctionArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.repay_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.bidder_debt_account.amount,
            args.repay_amount,
            ErrorCode::InsufficientBalance
        );
        let debt_asset = validate_liquidation_accounts(
            &self.market,
            self.bidder.key(),
            &self.debt_asset_mint,
            &self.collateral_asset_mint,
            &self.reserve_vault,
            &self.collateral_vault,
            &self.insurance_vault,
            &self.collateral_insurance_vault,
            &self.bidder_debt_account,
            &self.bidder_collateral_account,
        )?;
        let interest_asset =
            validate_interest_accounts(&self.market, &self.debt_asset_mint, &self.interest_vault)?;
        require!(interest_asset == debt_asset, ErrorCode::InvalidVault);
        require_supported_asset_mint(&self.debt_asset_mint)?;
        require_supported_asset_mint(&self.collateral_asset_mint)?;
        require_keys_eq!(
            self.margin_position.market,
            self.market.key(),
            ErrorCode::InvalidMarginPosition
        );
        self.liquidation_auction.assert_matches(
            self.market.key(),
            self.margin_position.key(),
            &self.margin_position,
            debt_asset,
            self.debt_asset_mint.key(),
            self.collateral_asset_mint.key(),
        )?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &SettleLiquidationAuctionArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_settle(ctx: Context<Self>, args: SettleLiquidationAuctionArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let margin_position_key = ctx.accounts.margin_position.key();
        let borrower_key = ctx.accounts.margin_position.owner;
        let bidder_key = ctx.accounts.bidder.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let collateral_asset_mint_key = ctx.accounts.collateral_asset_mint.key();
        let current_slot = Clock::get()?.slot;
        let debt_asset = ctx.accounts.market.asset_for_mint(debt_asset_mint_key)?;

        ctx.accounts.liquidation_auction.assert_matches(
            market_key,
            margin_position_key,
            &ctx.accounts.margin_position,
            debt_asset,
            debt_asset_mint_key,
            collateral_asset_mint_key,
        )?;
        let health_bps = ctx
            .accounts
            .market
            .position_health_bps(&ctx.accounts.margin_position, debt_asset)?;
        require!(
            health_bps < ctx.accounts.market.config.market_health_min_bps as u64,
            ErrorCode::PositionNotLiquidatable
        );
        let live_terms = ctx
            .accounts
            .market
            .liquidation_terms(&ctx.accounts.margin_position, debt_asset)?;
        let auction_incentive_bps = ctx
            .accounts
            .liquidation_auction
            .current_incentive_bps(current_slot, live_terms.liquidation_incentive_bps)?;
        let auction_pricing = LiquidationPricing::ReferencePrice {
            debt_per_collateral_price_nad: ctx.accounts.liquidation_auction.reference_price_nad,
        };
        let auction_terms = ctx
            .accounts
            .market
            .liquidation_terms_with_incentive_and_pricing(
                &ctx.accounts.margin_position,
                debt_asset,
                auction_incentive_bps,
                LiquidationPricing::PessimisticReserves,
            )?;
        require_gte!(
            auction_terms.max_repay_amount,
            args.repay_amount,
            ErrorCode::LiquidationRepayTooLarge
        );

        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let debt_transfer_fee = get_transfer_fee(
            &ctx.accounts.debt_asset_mint.to_account_info(),
            args.repay_amount,
        )?;
        let repay_credit = args
            .repay_amount
            .checked_sub(debt_transfer_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(repay_credit > 0, ErrorCode::AmountZero);
        transfer_from_user_to_vault(
            ctx.accounts.bidder.to_account_info(),
            ctx.accounts.bidder_debt_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program.clone(),
            args.repay_amount,
            ctx.accounts.debt_asset_mint.decimals,
        )?;

        let insurance_request = if args.max_insurance_draw > 0 {
            ctx.accounts
                .market
                .insurance_request_for_liquidation_with_terms_and_pricing(
                    &ctx.accounts.margin_position,
                    debt_asset,
                    repay_credit,
                    args.max_insurance_draw,
                    auction_terms,
                    auction_pricing,
                )?
        } else {
            0
        };
        let (insurance_spent, insurance_credit) = if insurance_request > 0 {
            let reserve_balance_before_insurance = ctx.accounts.reserve_vault.amount;
            let insurance_balance_before = ctx.accounts.insurance_vault.amount;
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.insurance_vault.to_account_info(),
                ctx.accounts.reserve_vault.to_account_info(),
                ctx.accounts.debt_asset_mint.to_account_info(),
                debt_token_program.clone(),
                insurance_request,
                ctx.accounts.debt_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.reserve_vault.reload()?;
            ctx.accounts.insurance_vault.reload()?;
            (
                insurance_balance_before
                    .checked_sub(ctx.accounts.insurance_vault.amount)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
                ctx.accounts
                    .reserve_vault
                    .amount
                    .checked_sub(reserve_balance_before_insurance)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
        } else {
            (0, 0)
        };

        let liquidation_receipt = ctx.accounts.market.settle_liquidation(
            &mut ctx.accounts.margin_position,
            debt_asset,
            repay_credit,
            insurance_spent,
            insurance_credit,
            args.max_socialized_loss,
            auction_terms,
            auction_pricing,
        )?;
        if liquidation_receipt.interest_paid > 0 {
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.reserve_vault.to_account_info(),
                ctx.accounts.interest_vault.to_account_info(),
                ctx.accounts.debt_asset_mint.to_account_info(),
                debt_token_program,
                liquidation_receipt.interest_paid,
                ctx.accounts.debt_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.interest_vault.reload()?;
            let manager_fee_bps = ctx.accounts.market.config.manager_fee_bps;
            ctx.accounts
                .market
                .side_mut(debt_asset)?
                .record_interest_credit(
                    liquidation_receipt.interest_paid,
                    manager_fee_bps,
                    ctx.accounts.futarchy_authority.revenue_share.interest_bps,
                    ctx.accounts.futarchy_authority.protocol_auction_split,
                )?;
        }

        let collateral_token_program = token_program_for_mint(
            &ctx.accounts.collateral_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let collateral_credit = if liquidation_receipt.collateral_to_liquidator > 0 {
            let transfer_fee = get_transfer_fee(
                &ctx.accounts.collateral_asset_mint.to_account_info(),
                liquidation_receipt.collateral_to_liquidator,
            )?;
            let collateral_credit = liquidation_receipt
                .collateral_to_liquidator
                .checked_sub(transfer_fee)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            require_gte!(
                collateral_credit,
                args.min_collateral_out,
                ErrorCode::SlippageExceeded
            );
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.collateral_vault.to_account_info(),
                ctx.accounts.bidder_collateral_account.to_account_info(),
                ctx.accounts.collateral_asset_mint.to_account_info(),
                collateral_token_program.clone(),
                liquidation_receipt.collateral_to_liquidator,
                ctx.accounts.collateral_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            collateral_credit
        } else {
            0
        };
        require_gte!(
            collateral_credit,
            args.min_collateral_out,
            ErrorCode::SlippageExceeded
        );
        if liquidation_receipt.insurance_funded > 0 {
            let collateral_insurance_balance_before =
                ctx.accounts.collateral_insurance_vault.amount;
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.collateral_vault.to_account_info(),
                ctx.accounts.collateral_insurance_vault.to_account_info(),
                ctx.accounts.collateral_asset_mint.to_account_info(),
                collateral_token_program,
                liquidation_receipt.insurance_funded,
                ctx.accounts.collateral_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.collateral_insurance_vault.reload()?;
            let insurance_credit = crate::instructions::common::token_account_credit(
                collateral_insurance_balance_before,
                &ctx.accounts.collateral_insurance_vault,
            )?;
            require_eq!(
                insurance_credit,
                liquidation_receipt.insurance_funded,
                ErrorCode::MarketMathOverflow
            );
        }

        ctx.accounts.liquidation_auction.record_settlement(
            &ctx.accounts.margin_position,
            liquidation_receipt.repaid_amount,
            current_slot,
            false,
        )?;

        emit!(LiquidationAuctionSettled {
            market: market_key,
            margin_position: margin_position_key,
            borrower: borrower_key,
            bidder: bidder_key,
            debt_asset_mint: debt_asset_mint_key,
            collateral_asset_mint: collateral_asset_mint_key,
            repaid_amount: liquidation_receipt.repaid_amount,
            collateral_to_bidder: liquidation_receipt.collateral_to_liquidator,
            collateral_seized: liquidation_receipt.collateral_seized,
            insurance_funded: liquidation_receipt.insurance_funded,
            insurance_drawn: liquidation_receipt.insurance_drawn,
            socialized_loss: liquidation_receipt.socialized_loss,
            auction_incentive_bps,
            max_repay_amount: liquidation_receipt.max_repay_amount,
            remaining_debt: liquidation_receipt.remaining_debt,
            auction_active: ctx.accounts.liquidation_auction.active,
            metadata: MarketEventMetadata::new(bidder_key, market_key)?,
        });
        Ok(())
    }
}
