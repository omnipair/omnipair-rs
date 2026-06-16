use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketClaimRedeemed, MarketEventMetadata},
    generate_market_seeds,
    shared::token::{token_burn, transfer_from_vault_to_user},
    state::Market,
    transitions::reserve::RemoveLiquidity as RemoveLiquidityTransition,
};

use crate::instructions::common::{
    require_fee_free_claim_token_mint, require_supported_asset_mint, token_account_credit,
    token_program_for_mint, validate_reserve_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemoveLiquidityArgs {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub min_asset_amount_out: u64,
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

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub claim_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> RemoveLiquidity<'info> {
    pub fn validate(&self, args: &RemoveLiquidityArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_claim_account.amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        validate_reserve_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_token_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        require_fee_free_claim_token_mint(&self.claim_token_mint)?;
        Ok(())
    }

    pub fn handle_remove_liquidity(ctx: Context<Self>, args: RemoveLiquidityArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        ctx.accounts
            .market
            .enforce_daily_withdraw_limit(args.market_side_index, args.claim_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_token_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            args.claim_amount,
            &[],
        )?;

        let owner_asset_balance_before = ctx.accounts.owner_asset_account.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.claim_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_asset_account.reload()?;
        let asset_credit = token_account_credit(
            owner_asset_balance_before,
            &ctx.accounts.owner_asset_account,
        )?;
        require_gte!(
            asset_credit,
            args.min_asset_amount_out,
            ErrorCode::SlippageExceeded
        );

        let receipt = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            RemoveLiquidityTransition::new(args.claim_amount).apply(market_side)?
        };
        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        emit_cpi!(MarketClaimRedeemed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            claim_amount: receipt.claim_amount,
            protected_claim_token_supply: receipt.protected_claim_token_supply,
            required_buffer: receipt.required_buffer,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}
