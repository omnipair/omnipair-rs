use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketDebtUpdated, MarketEventMetadata, MarketHealthUpdated},
    shared::token::transfer_from_user_to_vault,
    v2::state::{MarginPosition, Market},
};

use crate::v2::instructions::common::{require_supported_asset_mint, token_program_for_mint};

use super::common::{apply_repay_state, validate_repay_accounts};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MarketRepayArgs {
    pub repay_asset_is_asset0: bool,
    pub repay_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: MarketRepayArgs)]
pub struct MarketRepay<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

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

impl<'info> MarketRepay<'info> {
    pub fn validate(&self, args: &MarketRepayArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.repay_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_debt_account.amount,
            args.repay_amount,
            ErrorCode::InsufficientBalance
        );
        validate_repay_accounts(
            &self.market,
            args.repay_asset_is_asset0,
            self.owner.key(),
            &self.debt_asset_mint,
            &self.reserve_vault,
            &self.owner_debt_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        self.margin_position
            .assert_position(self.owner.key(), self.market.key())?;
        Ok(())
    }

    pub fn handle_repay(ctx: Context<Self>, args: MarketRepayArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let reserve_balance_before = ctx.accounts.reserve_vault.amount;
        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_debt_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program,
            args.repay_amount,
            ctx.accounts.debt_asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;
        let repay_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(repay_credit > 0, ErrorCode::AmountZero);
        let debt_delta = -i64::try_from(repay_credit).map_err(|_| ErrorCode::Overflow)?;

        apply_repay_state(
            &mut ctx.accounts.market,
            &mut ctx.accounts.margin_position,
            args.repay_asset_is_asset0,
            repay_credit,
        )?;

        emit_cpi!(MarketDebtUpdated {
            market: market_key,
            owner: owner_key,
            debt_asset_mint: debt_asset_mint_key,
            debt_delta,
            fixed_debt0: ctx.accounts.market.debt_book.fixed_debt0()?,
            fixed_debt1: ctx.accounts.market.debt_book.fixed_debt1()?,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key),
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
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });
        Ok(())
    }
}
