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
    state::{MarginPosition, Market},
    transitions::debt::Borrow as BorrowTransition,
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
};

use super::common::validate_borrow_accounts;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct BorrowArgs {
    pub borrow_asset_is_asset0: bool,
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
        self.market.assert_live()?;
        require!(
            !self.market.config.soft_borrow_enabled,
            ErrorCode::InvalidMarketConfig
        );
        require!(args.borrow_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            args.borrow_amount,
            args.min_debt_amount_out,
            ErrorCode::SlippageExceeded
        );
        validate_borrow_accounts(
            &self.market,
            args.borrow_asset_is_asset0,
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

    pub fn handle_borrow(ctx: Context<Self>, args: BorrowArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();

        let debt_receipt = BorrowTransition::new(
            args.borrow_asset_is_asset0,
            args.borrow_amount,
            args.min_health_bps,
        )
        .apply(&mut ctx.accounts.market, &mut ctx.accounts.margin_position)?;

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
            fixed_debt0: debt_receipt.fixed_debt0,
            fixed_debt1: debt_receipt.fixed_debt1,
            health0_bps: debt_receipt.health0_bps,
            health1_bps: debt_receipt.health1_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        emit_cpi!(MarketHealthUpdated {
            market: market_key,
            recognized_collateral0_for_debt1: ctx
                .accounts
                .market
                .health
                .recognized_collateral0_for_debt1,
            recognized_collateral1_for_debt0: ctx
                .accounts
                .market
                .health
                .recognized_collateral1_for_debt0,
            effective_debt0_nad: ctx.accounts.market.health.effective_debt0_nad,
            effective_debt1_nad: ctx.accounts.market.health.effective_debt1_nad,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        Ok(())
    }
}
