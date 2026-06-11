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
    v2::state::Market,
};

use crate::v2::instructions::common::{
    require_fee_free_claim_mint, require_supported_asset_mint, token_program_for_mint,
    validate_reserve_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RedeemClaimArgs {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub min_asset_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: RedeemClaimArgs)]
pub struct RedeemClaim<'info> {
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

    #[account(mut)]
    pub claim_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> RedeemClaim<'info> {
    pub fn validate(&self, args: &RedeemClaimArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_claim_account.amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            args.claim_amount,
            args.min_asset_amount_out,
            ErrorCode::SlippageExceeded
        );
        validate_reserve_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        require_fee_free_claim_mint(&self.claim_mint)?;
        Ok(())
    }

    pub fn handle_redeem(ctx: Context<Self>, args: RedeemClaimArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        ctx.accounts
            .market
            .enforce_daily_withdraw_limit(args.market_side_index, args.claim_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            args.claim_amount,
            &[],
        )?;

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

        let (protected_claim_supply, required_buffer) = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.apply_claim_redemption(args.claim_amount)?;
            (
                market_side.claim_ledger.protected_claim_supply,
                market_side.buffer_book.required_buffer,
            )
        };
        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_spot_ema_divergence()?;

        emit_cpi!(MarketClaimRedeemed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            claim_amount: args.claim_amount,
            protected_claim_supply,
            required_buffer,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}
