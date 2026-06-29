use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{LiquidityRemoved, MarketEventMetadata, MarketHealthUpdated},
    generate_market_seeds,
    shared::token::{token_burn, transfer_from_vault_to_user},
    state::{Market, YieldAccount, YieldTokenKind},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint, validate_lp_mint,
    validate_owner_asset_account, validate_owner_lp_account, validate_side_vault_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemoveLiquidityArgs {
    pub ylp_amount: u64,
    pub min_base_amount_out: u64,
    pub min_quote_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: RemoveLiquidityArgs)]
pub struct RemoveLiquidity<'info> {
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

    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub ylp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub base_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub quote_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_base_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub owner_quote_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub owner_ylp_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            YIELD_ACCOUNT_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            base_mint.key().as_ref(),
            &[YieldTokenKind::Ylp.code()],
        ],
        bump = base_yield_account.bump
    )]
    pub base_yield_account: Box<Account<'info, YieldAccount>>,

    #[account(
        mut,
        seeds = [
            YIELD_ACCOUNT_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            quote_mint.key().as_ref(),
            &[YieldTokenKind::Ylp.code()],
        ],
        bump = quote_yield_account.bump
    )]
    pub quote_yield_account: Box<Account<'info, YieldAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> RemoveLiquidity<'info> {
    pub fn validate(&self, args: &RemoveLiquidityArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.ylp_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_ylp_account.amount,
            args.ylp_amount,
            ErrorCode::InsufficientBalance
        );
        validate_side_vault_accounts(
            &self.market,
            crate::state::MarketAsset::Base,
            &self.base_mint,
            &self.base_reserve_vault,
        )?;
        validate_side_vault_accounts(
            &self.market,
            crate::state::MarketAsset::Quote,
            &self.quote_mint,
            &self.quote_reserve_vault,
        )?;
        require_keys_eq!(
            self.market.ylp_mint,
            self.ylp_mint.key(),
            ErrorCode::InvalidLpMintKey
        );
        validate_owner_asset_account(self.owner.key(), &self.base_mint, &self.owner_base_account)?;
        validate_owner_asset_account(
            self.owner.key(),
            &self.quote_mint,
            &self.owner_quote_account,
        )?;
        validate_owner_lp_account(self.owner.key(), &self.ylp_mint, &self.owner_ylp_account)?;
        require_supported_asset_mint(&self.base_mint)?;
        require_supported_asset_mint(&self.quote_mint)?;
        validate_lp_mint(&self.ylp_mint, self.market.key(), self.base_mint.decimals)?;
        self.base_yield_account.assert_account(
            self.owner.key(),
            self.market.key(),
            self.base_mint.key(),
            YieldTokenKind::Ylp,
        )?;
        self.quote_yield_account.assert_account(
            self.owner.key(),
            self.market.key(),
            self.quote_mint.key(),
            YieldTokenKind::Ylp,
        )?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &RemoveLiquidityArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_remove_liquidity(ctx: Context<Self>, args: RemoveLiquidityArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();

        {
            let market = &mut ctx.accounts.market;
            market.base_side.carry_forward_swap_fees()?;
            market.base_side.carry_forward_interest()?;
            market.quote_side.carry_forward_swap_fees()?;
            market.quote_side.carry_forward_interest()?;
            ctx.accounts.base_yield_account.accrue(
                ctx.accounts.owner_ylp_account.amount,
                market.base_side.fees.swap_fee_growth_index_nad,
                market.base_side.fees.interest_growth_index_nad,
            )?;
            ctx.accounts.quote_yield_account.accrue(
                ctx.accounts.owner_ylp_account.amount,
                market.quote_side.fees.swap_fee_growth_index_nad,
                market.quote_side.fees.interest_growth_index_nad,
            )?;
        }

        let ylp_program = token_program_for_mint(
            &ctx.accounts.ylp_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            ylp_program,
            ctx.accounts.ylp_mint.to_account_info(),
            ctx.accounts.owner_ylp_account.to_account_info(),
            args.ylp_amount,
            &[],
        )?;

        let receipt = ctx.accounts.market.remove_liquidity(args.ylp_amount)?;
        ctx.accounts.market.enforce_daily_withdraw_limit(
            crate::state::MarketAsset::Base,
            receipt.base_amount_out,
        )?;
        ctx.accounts.market.enforce_daily_withdraw_limit(
            crate::state::MarketAsset::Quote,
            receipt.quote_amount_out,
        )?;

        let base_balance_before = ctx.accounts.owner_base_account.amount;
        let quote_balance_before = ctx.accounts.owner_quote_account.amount;
        let base_token_program = token_program_for_mint(
            &ctx.accounts.base_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let quote_token_program = token_program_for_mint(
            &ctx.accounts.quote_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.base_reserve_vault.to_account_info(),
            ctx.accounts.owner_base_account.to_account_info(),
            ctx.accounts.base_mint.to_account_info(),
            base_token_program,
            receipt.base_amount_out,
            ctx.accounts.base_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.quote_reserve_vault.to_account_info(),
            ctx.accounts.owner_quote_account.to_account_info(),
            ctx.accounts.quote_mint.to_account_info(),
            quote_token_program,
            receipt.quote_amount_out,
            ctx.accounts.quote_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_base_account.reload()?;
        ctx.accounts.owner_quote_account.reload()?;
        let base_credit =
            token_account_credit(base_balance_before, &ctx.accounts.owner_base_account)?;
        let quote_credit =
            token_account_credit(quote_balance_before, &ctx.accounts.owner_quote_account)?;
        require_gte!(
            base_credit,
            args.min_base_amount_out,
            ErrorCode::SlippageExceeded
        );
        require_gte!(
            quote_credit,
            args.min_quote_amount_out,
            ErrorCode::SlippageExceeded
        );
        ctx.accounts.market.refresh_risk()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        emit_cpi!(LiquidityRemoved {
            market: market_key,
            owner: owner_key,
            ylp_amount: receipt.ylp_amount,
            base_amount_out: receipt.base_amount_out,
            quote_amount_out: receipt.quote_amount_out,
            ylp_supply: receipt.ylp_supply,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        emit_cpi!(MarketHealthUpdated {
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
