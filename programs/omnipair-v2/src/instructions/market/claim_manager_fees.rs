use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{ManagerFeesClaimed, MarketEventMetadata},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{Market, MarketAsset},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_program_for_mint, validate_fee_accounts,
    validate_interest_accounts, validate_owner_asset_account,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimManagerFeesArgs {
    pub side: MarketAsset,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimManagerFeesArgs)]
pub struct ClaimManagerFees<'info> {
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
    pub manager: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub interest_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub manager_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimManagerFees<'info> {
    pub fn validate(&self, args: &ClaimManagerFeesArgs) -> Result<()> {
        self.market.assert_manager(self.manager.key())?;
        validate_fee_accounts(&self.market, args.side, &self.asset_mint, &self.fee_vault)?;
        validate_interest_accounts(
            &self.market,
            args.side,
            &self.asset_mint,
            &self.interest_vault,
        )?;
        validate_owner_asset_account(
            self.manager.key(),
            &self.asset_mint,
            &self.manager_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimManagerFeesArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let manager_key = ctx.accounts.manager.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();
        let (swap_fee_amount, interest_fee_amount) = {
            let market_side = ctx.accounts.market.side(args.side)?;
            (
                market_side.fees.manager_swap_fee_liability,
                market_side.fees.manager_interest_fee_liability,
            )
        };
        require!(
            swap_fee_amount > 0 || interest_fee_amount > 0,
            ErrorCode::AmountZero
        );

        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        if swap_fee_amount > 0 {
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.fee_vault.to_account_info(),
                ctx.accounts.manager_asset_account.to_account_info(),
                ctx.accounts.asset_mint.to_account_info(),
                asset_token_program.clone(),
                swap_fee_amount,
                ctx.accounts.asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
        }
        if interest_fee_amount > 0 {
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.interest_vault.to_account_info(),
                ctx.accounts.manager_asset_account.to_account_info(),
                ctx.accounts.asset_mint.to_account_info(),
                asset_token_program,
                interest_fee_amount,
                ctx.accounts.asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
        }

        ctx.accounts.fee_vault.reload()?;
        ctx.accounts.interest_vault.reload()?;
        {
            let market_side = ctx.accounts.market.side_mut(args.side)?;
            market_side.fees.manager_swap_fee_liability = 0;
            market_side.fees.manager_interest_fee_liability = 0;
            market_side.fees.swap_fee_vault_balance = ctx.accounts.fee_vault.amount;
            market_side.fees.interest_vault_balance = ctx.accounts.interest_vault.amount;
            market_side.fees.assert_backed()?;
        }

        emit_cpi!(ManagerFeesClaimed {
            market: market_key,
            manager: manager_key,
            asset_mint: asset_mint_key,
            swap_fee_amount,
            interest_fee_amount,
            remaining_manager_swap_fee_liability: 0,
            remaining_manager_interest_fee_liability: 0,
            metadata: MarketEventMetadata::new(manager_key, market_key)?,
        });

        Ok(())
    }
}
