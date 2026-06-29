use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketCollateralWithdrawn, MarketEventMetadata, MarketHealthUpdated},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{FutarchyAuthority, MarginPosition, Market},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_account_debit, token_program_for_mint,
};

use super::common::validate_collateral_accounts;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct WithdrawCollateralArgs {
    pub withdraw_amount: u64,
    pub min_asset_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: WithdrawCollateralArgs)]
pub struct WithdrawCollateral<'info> {
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

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

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

impl<'info> WithdrawCollateral<'info> {
    pub fn validate(&self, args: &WithdrawCollateralArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.withdraw_amount > 0, ErrorCode::AmountZero);
        let market_asset = validate_collateral_accounts(
            &self.market,
            self.owner.key(),
            &self.asset_mint,
            &self.collateral_vault,
            &self.owner_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        self.margin_position
            .assert_position(self.owner.key(), self.market.key())?;
        if self
            .futarchy_authority
            .is_reduce_only(self.market.reduce_only)
        {
            let debt = self
                .margin_position
                .fixed_base_debt(&self.market.debt)?
                .checked_add(self.margin_position.fixed_quote_debt(&self.market.debt)?)
                .ok_or(ErrorCode::DebtMathOverflow)?;
            require!(debt == 0, ErrorCode::ReduceOnlyHasDebt);
        }
        require_gte!(
            self.collateral_vault.amount,
            args.withdraw_amount,
            ErrorCode::InsufficientBalance
        );
        let idle_collateral = match market_asset {
            crate::state::MarketAsset::Base => self.margin_position.idle_base_collateral()?,
            crate::state::MarketAsset::Quote => self.margin_position.idle_quote_collateral()?,
        };
        require_gte!(
            idle_collateral,
            args.withdraw_amount,
            ErrorCode::InsufficientRecognizedCollateral
        );
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &WithdrawCollateralArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_withdraw(ctx: Context<Self>, args: WithdrawCollateralArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();
        let market_asset = ctx.accounts.market.asset_for_mint(asset_mint_key)?;
        let owner_asset_balance_before = ctx.accounts.owner_asset_account.amount;
        let collateral_balance_before = ctx.accounts.collateral_vault.amount;

        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.collateral_vault.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.withdraw_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_asset_account.reload()?;
        ctx.accounts.collateral_vault.reload()?;
        let asset_credit = token_account_credit(
            owner_asset_balance_before,
            &ctx.accounts.owner_asset_account,
        )?;
        let collateral_debit =
            token_account_debit(collateral_balance_before, &ctx.accounts.collateral_vault)?;
        require_eq!(
            collateral_debit,
            args.withdraw_amount,
            ErrorCode::MarketMathOverflow
        );
        require_gte!(
            asset_credit,
            args.min_asset_amount_out,
            ErrorCode::SlippageExceeded
        );

        let collateral_receipt = ctx.accounts.market.withdraw_collateral(
            &mut ctx.accounts.margin_position,
            market_asset,
            collateral_debit,
        )?;

        emit_cpi!(MarketCollateralWithdrawn {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            collateral_debit: collateral_receipt.collateral_debit,
            asset_credit,
            base_collateral: collateral_receipt.base_collateral,
            quote_collateral: collateral_receipt.quote_collateral,
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
