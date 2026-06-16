use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketCollateralDeposited, MarketEventMetadata},
    shared::{account::get_size_with_discriminator, token::transfer_from_user_to_vault},
    state::{MarginPosition, Market},
};

use crate::instructions::common::{require_supported_asset_mint, token_program_for_mint};

use super::common::validate_collateral_accounts;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositCollateralArgs {
    pub market_side_index: u8,
    pub deposit_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DepositCollateralArgs)]
pub struct DepositCollateral<'info> {
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

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<MarginPosition>(),
        seeds = [
            MARGIN_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump
    )]
    pub margin_position: Box<Account<'info, MarginPosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> DepositCollateral<'info> {
    pub fn validate(&self, args: &DepositCollateralArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_collateral_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.collateral_vault,
            &self.owner_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        if self.margin_position.is_initialized() {
            self.margin_position
                .assert_position(self.owner.key(), self.market.key())?;
        }
        Ok(())
    }

    pub fn handle_deposit(ctx: Context<Self>, args: DepositCollateralArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        if !ctx.accounts.margin_position.is_initialized() {
            ctx.accounts.margin_position.initialize(
                owner_key,
                market_key,
                ctx.bumps.margin_position,
            );
        }
        ctx.accounts
            .margin_position
            .assert_position(owner_key, market_key)?;

        let collateral_balance_before = ctx.accounts.collateral_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.collateral_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.collateral_vault.reload()?;
        let collateral_credit = ctx
            .accounts
            .collateral_vault
            .amount
            .checked_sub(collateral_balance_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(collateral_credit > 0, ErrorCode::AmountZero);

        if args.market_side_index == 0 {
            ctx.accounts.margin_position.collateral0 = ctx
                .accounts
                .margin_position
                .collateral0
                .checked_add(collateral_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        } else {
            ctx.accounts.margin_position.collateral1 = ctx
                .accounts
                .margin_position
                .collateral1
                .checked_add(collateral_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }

        emit_cpi!(MarketCollateralDeposited {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            collateral_credit,
            collateral0: ctx.accounts.margin_position.collateral0,
            collateral1: ctx.accounts.margin_position.collateral1,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}
