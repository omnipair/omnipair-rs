use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketFeeLiabilityClaimed},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{Market, MarketAsset, MarketFeeClaimKind},
    transitions::fee::{PrepareMarketFeeClaim, SettleMarketFeeClaim},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
    validate_fee_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimMarketFeesArgs {
    pub market_asset: MarketAsset,
    pub claim_kind: MarketFeeClaimKind,
    pub min_fee_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimMarketFeesArgs)]
pub struct ClaimMarketFees<'info> {
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
    pub fee_authority: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub recipient_fee_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimMarketFees<'info> {
    pub fn validate(&self, args: &ClaimMarketFeesArgs) -> Result<()> {
        self.market.assert_started()?;
        match args.claim_kind {
            MarketFeeClaimKind::Operator => require_keys_eq!(
                self.fee_authority.key(),
                self.market.operator,
                ErrorCode::InvalidMarketFeeAuthority
            ),
            MarketFeeClaimKind::Protocol => require_keys_eq!(
                self.fee_authority.key(),
                self.market.manager,
                ErrorCode::InvalidMarketFeeAuthority
            ),
        }
        validate_fee_accounts(
            &self.market,
            args.market_asset,
            self.fee_authority.key(),
            &self.asset_mint,
            &self.fee_vault,
            &self.recipient_fee_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimMarketFeesArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let fee_authority_key = ctx.accounts.fee_authority.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        let pending_claim = {
            let market_side = ctx.accounts.market.side(args.market_asset)?;
            PrepareMarketFeeClaim::new(args.claim_kind, ctx.accounts.fee_vault.amount)
                .apply(market_side)?
        };

        let recipient_fee_balance_before = ctx.accounts.recipient_fee_account.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.fee_vault.to_account_info(),
            ctx.accounts.recipient_fee_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            pending_claim.fee_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.recipient_fee_account.reload()?;
        ctx.accounts.fee_vault.reload()?;
        let fee_credit = token_account_credit(
            recipient_fee_balance_before,
            &ctx.accounts.recipient_fee_account,
        )?;
        require_gte!(fee_credit, args.min_fee_amount, ErrorCode::SlippageExceeded);

        let settled_claim = {
            let market_side = ctx.accounts.market.side_mut(args.market_asset)?;
            SettleMarketFeeClaim::new(
                args.claim_kind,
                pending_claim.fee_amount,
                ctx.accounts.fee_vault.amount,
            )
            .apply(market_side)?
        };

        emit_cpi!(MarketFeeLiabilityClaimed {
            market: market_key,
            authority: fee_authority_key,
            asset_mint: asset_mint_key,
            claim_kind: args.claim_kind.event_code(),
            fee_amount: settled_claim.fee_amount,
            remaining_fee_liability: settled_claim.remaining_fee_liability,
            metadata: MarketEventMetadata::new(fee_authority_key, market_key)?,
        });

        Ok(())
    }
}
