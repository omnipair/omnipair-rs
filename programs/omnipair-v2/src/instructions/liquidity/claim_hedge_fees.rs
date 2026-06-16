use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketHedgeFeesClaimed},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{HedgePosition, Market},
    transitions::fee::{PrepareHedgedFeeClaim, SettleHedgedFeeClaim},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
    validate_fee_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimHedgeFeesArgs {
    pub market_side_index: u8,
    pub min_fee_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimHedgeFeesArgs)]
pub struct ClaimHedgeFees<'info> {
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
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_fee_account: Box<InterfaceAccount<'info, TokenAccount>>,

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

impl<'info> ClaimHedgeFees<'info> {
    pub fn validate(&self, args: &ClaimHedgeFeesArgs) -> Result<()> {
        self.market.assert_started()?;
        validate_fee_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.fee_vault,
            &self.owner_fee_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        self.hedge_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimHedgeFeesArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        let pending_claim = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            PrepareHedgedFeeClaim::new(ctx.accounts.fee_vault.amount)
                .apply(market_side, &mut ctx.accounts.hedge_position)?
        };

        let owner_fee_balance_before = ctx.accounts.owner_fee_account.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.fee_vault.to_account_info(),
            ctx.accounts.owner_fee_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            pending_claim.fee_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_fee_account.reload()?;
        ctx.accounts.fee_vault.reload()?;
        let fee_credit =
            token_account_credit(owner_fee_balance_before, &ctx.accounts.owner_fee_account)?;
        require_gte!(fee_credit, args.min_fee_amount, ErrorCode::SlippageExceeded);

        let settled_claim = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            SettleHedgedFeeClaim::new(pending_claim.fee_amount, ctx.accounts.fee_vault.amount)
                .apply(market_side, &mut ctx.accounts.hedge_position)?
        };

        emit_cpi!(MarketHedgeFeesClaimed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            fee_amount: settled_claim.fee_amount,
            remaining_fee_liability: settled_claim.remaining_fee_liability,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}
