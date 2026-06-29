use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, YieldClaimed},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{Market, MarketAsset, YieldAccount, YieldClaimReceipt, YieldTokenKind},
};

use crate::instructions::common::{
    token_program_for_mint, validate_fee_accounts, validate_interest_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimYieldArgs {
    pub token_kind: YieldTokenKind,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimYieldArgs)]
pub struct ClaimYield<'info> {
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
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub owner_lp_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub interest_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub recipient_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            YIELD_ACCOUNT_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
            &[args.token_kind.code()],
        ],
        bump = yield_account.bump
    )]
    pub yield_account: Box<Account<'info, YieldAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimYield<'info> {
    pub fn validate(&self, args: &ClaimYieldArgs) -> Result<()> {
        let market_asset = self.market.asset_for_mint(self.asset_mint.key())?;
        let market_side = self.market.side(market_asset)?;
        require_keys_eq!(
            market_side.asset_mint,
            self.asset_mint.key(),
            ErrorCode::InvalidMint
        );
        match args.token_kind {
            YieldTokenKind::Ylp => {
                require_keys_eq!(
                    self.market.ylp_mint,
                    self.lp_mint.key(),
                    ErrorCode::InvalidMint
                )
            }
            YieldTokenKind::Hlp => {
                require_keys_eq!(
                    market_side.hlp_mint,
                    self.lp_mint.key(),
                    ErrorCode::InvalidMint
                )
            }
        }
        require_keys_eq!(
            self.owner_lp_account.mint,
            self.lp_mint.key(),
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            self.owner_lp_account.owner,
            self.owner.key(),
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            self.recipient_asset_account.owner,
            self.yield_account.recipient,
            ErrorCode::InvalidRecipient
        );
        require_keys_eq!(
            self.recipient_asset_account.mint,
            self.asset_mint.key(),
            ErrorCode::InvalidTokenAccount
        );
        let fee_asset = validate_fee_accounts(&self.market, &self.asset_mint, &self.fee_vault)?;
        let interest_asset =
            validate_interest_accounts(&self.market, &self.asset_mint, &self.interest_vault)?;
        require!(fee_asset == market_asset, ErrorCode::InvalidVault);
        require!(interest_asset == market_asset, ErrorCode::InvalidVault);
        self.yield_account.assert_account(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
            args.token_kind,
        )
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &ClaimYieldArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimYieldArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();
        let market_asset = ctx.accounts.market.asset_for_mint(asset_mint_key)?;
        let token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let vault_balance = ctx
            .accounts
            .fee_vault
            .amount
            .checked_add(ctx.accounts.interest_vault.amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let receipt = match args.token_kind {
            YieldTokenKind::Ylp => {
                let market_side = ctx.accounts.market.side_mut(market_asset)?;
                market_side.prepare_yield_claim(
                    &mut ctx.accounts.yield_account,
                    vault_balance,
                    ctx.accounts.owner_lp_account.amount,
                )?
            }
            YieldTokenKind::Hlp => {
                ctx.accounts
                    .market
                    .checkpoint_hlp_yield_from_ylp(market_asset)?;
                let (swap_fee_growth_index_nad, interest_growth_index_nad) =
                    hlp_yield_growth_indexes(&ctx.accounts.market, market_asset);
                ctx.accounts.yield_account.accrue(
                    ctx.accounts.owner_lp_account.amount,
                    swap_fee_growth_index_nad,
                    interest_growth_index_nad,
                )?;
                let claim_amount = ctx.accounts.yield_account.claimable_amount()?;
                require!(claim_amount > 0, ErrorCode::AmountZero);
                require_gte!(vault_balance, claim_amount, ErrorCode::UnbackedFeeLiability);
                YieldClaimReceipt {
                    claim_amount,
                    swap_fee_amount: ctx.accounts.yield_account.accrued_swap_fee_amount,
                    interest_amount: ctx.accounts.yield_account.accrued_interest_amount,
                    remaining_swap_fee_liability: ctx
                        .accounts
                        .market
                        .side(market_asset)?
                        .fees
                        .swap_fee_liability,
                    remaining_interest_liability: ctx
                        .accounts
                        .market
                        .side(market_asset)?
                        .fees
                        .interest_liability,
                }
            }
        };
        if receipt.swap_fee_amount > 0 {
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.fee_vault.to_account_info(),
                ctx.accounts.recipient_asset_account.to_account_info(),
                ctx.accounts.asset_mint.to_account_info(),
                token_program.clone(),
                receipt.swap_fee_amount,
                ctx.accounts.asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
        }
        if receipt.interest_amount > 0 {
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.interest_vault.to_account_info(),
                ctx.accounts.recipient_asset_account.to_account_info(),
                ctx.accounts.asset_mint.to_account_info(),
                token_program,
                receipt.interest_amount,
                ctx.accounts.asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
        }
        ctx.accounts.fee_vault.reload()?;
        ctx.accounts.interest_vault.reload()?;
        {
            let market_side = ctx.accounts.market.side_mut(market_asset)?;
            market_side.settle_yield_claim(
                &mut ctx.accounts.yield_account,
                receipt.claim_amount,
                receipt.swap_fee_amount,
                receipt.interest_amount,
                ctx.accounts.fee_vault.amount,
                ctx.accounts.interest_vault.amount,
            )?;
        }
        emit_cpi!(YieldClaimed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            token_kind: args.token_kind.code(),
            recipient: ctx.accounts.yield_account.recipient,
            swap_fee_amount: receipt.swap_fee_amount,
            interest_amount: receipt.interest_amount,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        Ok(())
    }
}

fn hlp_yield_growth_indexes(market: &Market, market_asset: MarketAsset) -> (u128, u128) {
    match market_asset {
        MarketAsset::Base => market
            .base_hlp_vault
            .yield_growth_indexes(MarketAsset::Base),
        MarketAsset::Quote => market
            .quote_hlp_vault
            .yield_growth_indexes(MarketAsset::Quote),
    }
}
