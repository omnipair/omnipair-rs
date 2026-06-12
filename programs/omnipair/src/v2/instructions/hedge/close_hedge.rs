use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketHedgeClosed},
    generate_market_seeds,
    shared::token::{token_burn, transfer_from_vault_to_user},
    v2::state::{HedgePosition, Market},
};

use super::common::validate_hedge_accounts;
use crate::v2::instructions::common::{token_account_credit, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CloseHedgeArgs {
    pub market_side_index: u8,
    pub hedge_amount: u64,
    pub min_claim_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: CloseHedgeArgs)]
pub struct CloseHedge<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub claim_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_hedge_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            HEDGE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = hedge_position.bump
    )]
    pub hedge_position: Box<Account<'info, HedgePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> CloseHedge<'info> {
    pub fn validate(&self, args: &CloseHedgeArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.hedge_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_hedge_account.amount,
            args.hedge_amount,
            ErrorCode::InsufficientBalance
        );
        validate_hedge_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.hedge_mint,
            &self.hedge_vault,
            &self.owner_claim_account,
            &self.owner_hedge_account,
        )?;
        self.hedge_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )?;
        require_gte!(
            self.hedge_position.hedged_claim_amount,
            args.hedge_amount,
            ErrorCode::InvalidHedgePosition
        );
        Ok(())
    }

    pub fn handle_close(ctx: Context<Self>, args: CloseHedgeArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let hedge_token_program = token_program_for_mint(
            &ctx.accounts.hedge_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            hedge_token_program,
            ctx.accounts.hedge_mint.to_account_info(),
            ctx.accounts.owner_hedge_account.to_account_info(),
            args.hedge_amount,
            &[],
        )?;

        let hedged_claim_supply = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.claim_ledger.hedged_claim_supply = market_side
                .claim_ledger
                .hedged_claim_supply
                .checked_sub(args.hedge_amount)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market_side.claim_ledger.hedged_claim_supply
        };
        ctx.accounts.hedge_position.decrease(args.hedge_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let owner_claim_balance_before = ctx.accounts.owner_claim_account.amount;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.hedge_vault.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.claim_mint.to_account_info(),
            claim_token_program,
            args.hedge_amount,
            ctx.accounts.claim_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_claim_account.reload()?;
        let claim_credit = token_account_credit(
            owner_claim_balance_before,
            &ctx.accounts.owner_claim_account,
        )?;
        require_gte!(
            claim_credit,
            args.min_claim_amount_out,
            ErrorCode::SlippageExceeded
        );

        emit_cpi!(MarketHedgeClosed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            hedge_amount: args.hedge_amount,
            claim_amount: args.hedge_amount,
            hedged_claim_supply,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}
