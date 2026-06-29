use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketDebtUpdated, MarketEventMetadata, MarketHealthUpdated},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{FutarchyAuthority, MarginPosition, Market},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
};

use super::common::validate_borrow_accounts;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct BorrowArgs {
    pub borrow_amount: u64,
    pub min_debt_amount_out: u64,
    pub min_health_bps: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: BorrowArgs)]
pub struct Borrow<'info> {
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
    pub owner: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Borrow<'info> {
    pub fn validate(&self, args: &BorrowArgs) -> Result<()> {
        self.market
            .assert_live_with_futarchy(&self.futarchy_authority)?;
        require!(args.borrow_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            args.borrow_amount,
            args.min_debt_amount_out,
            ErrorCode::SlippageExceeded
        );
        validate_borrow_accounts(
            &self.market,
            self.owner.key(),
            &self.debt_asset_mint,
            &self.collateral_asset_mint,
            &self.reserve_vault,
            &self.owner_debt_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        self.margin_position
            .assert_position(self.owner.key(), self.market.key())?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &BorrowArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_borrow(ctx: Context<Self>, args: BorrowArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let borrow_asset = ctx.accounts.market.asset_for_mint(debt_asset_mint_key)?;

        let debt_receipt = ctx.accounts.market.borrow(
            &mut ctx.accounts.margin_position,
            borrow_asset,
            args.borrow_amount,
            args.min_health_bps,
        )?;

        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let owner_debt_balance_before = ctx.accounts.owner_debt_account.amount;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.owner_debt_account.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program,
            args.borrow_amount,
            ctx.accounts.debt_asset_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_debt_account.reload()?;
        let debt_credit =
            token_account_credit(owner_debt_balance_before, &ctx.accounts.owner_debt_account)?;
        require_gte!(
            debt_credit,
            args.min_debt_amount_out,
            ErrorCode::SlippageExceeded
        );

        emit_cpi!(MarketDebtUpdated {
            market: market_key,
            owner: owner_key,
            debt_asset_mint: debt_asset_mint_key,
            debt_delta: debt_receipt.debt_delta,
            fixed_base_debt: debt_receipt.fixed_base_debt,
            fixed_quote_debt: debt_receipt.fixed_quote_debt,
            base_debt_health_bps: debt_receipt.base_debt_health_bps,
            quote_debt_health_bps: debt_receipt.quote_debt_health_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        emit!(MarketHealthUpdated {
            market: market_key,
            recognized_base_collateral_for_quote_debt: ctx
                .accounts
                .market
                .health
                .recognized_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: ctx
                .accounts
                .market
                .health
                .recognized_quote_collateral_for_base_debt,
            effective_base_debt_nad: ctx.accounts.market.health.effective_base_debt_nad,
            effective_quote_debt_nad: ctx.accounts.market.health.effective_quote_debt_nad,
            base_debt_health_bps: ctx.accounts.market.health.base_debt_health_bps,
            quote_debt_health_bps: ctx.accounts.market.health.quote_debt_health_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        Ok(())
    }
}
