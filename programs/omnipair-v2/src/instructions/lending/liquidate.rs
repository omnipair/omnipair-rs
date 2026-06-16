use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketLiquidated},
    generate_market_seeds,
    shared::token::{
        transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
    },
    state::{MarginPosition, Market},
    transitions::liquidation::{insurance_request_for_liquidation, Liquidation},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MarketLiquidateArgs {
    pub debt_asset_is_asset0: bool,
    pub repay_amount: u64,
    pub min_collateral_out: u64,
    pub max_insurance_draw: u64,
    pub max_socialized_loss: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: MarketLiquidateArgs)]
pub struct MarketLiquidate<'info> {
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
    pub liquidator: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub liquidator_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.owner.as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> MarketLiquidate<'info> {
    pub fn validate(&self, args: &MarketLiquidateArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.repay_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.liquidator_debt_account.amount,
            args.repay_amount,
            ErrorCode::InsufficientBalance
        );
        validate_liquidation_accounts(
            &self.market,
            args.debt_asset_is_asset0,
            self.liquidator.key(),
            &self.debt_asset_mint,
            &self.collateral_asset_mint,
            &self.reserve_vault,
            &self.collateral_vault,
            &self.insurance_vault,
            &self.liquidator_debt_account,
            &self.liquidator_collateral_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        require_supported_asset_mint(&self.collateral_asset_mint)?;
        require_keys_eq!(
            self.margin_position.market,
            self.market.key(),
            ErrorCode::InvalidMarginPosition
        );
        let health_bps = self
            .market
            .position_health_bps(&self.margin_position, args.debt_asset_is_asset0)?;
        require!(
            health_bps < self.market.config.market_health_min_bps as u64,
            ErrorCode::PositionNotLiquidatable
        );
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>, args: MarketLiquidateArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let borrower_key = ctx.accounts.margin_position.owner;
        let liquidator_key = ctx.accounts.liquidator.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let collateral_asset_mint_key = ctx.accounts.collateral_asset_mint.key();

        let reserve_balance_before_repay = ctx.accounts.reserve_vault.amount;
        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.liquidator.to_account_info(),
            ctx.accounts.liquidator_debt_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program.clone(),
            args.repay_amount,
            ctx.accounts.debt_asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;
        let repay_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before_repay)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(repay_credit > 0, ErrorCode::AmountZero);

        let insurance_request = insurance_request_for_liquidation(
            &ctx.accounts.market,
            &ctx.accounts.margin_position,
            args.debt_asset_is_asset0,
            repay_credit,
            args.max_insurance_draw,
        )?;

        let (insurance_spent, insurance_credit) = if insurance_request > 0 {
            let reserve_balance_before_insurance = ctx.accounts.reserve_vault.amount;
            let insurance_balance_before = ctx.accounts.insurance_vault.amount;
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.insurance_vault.to_account_info(),
                ctx.accounts.reserve_vault.to_account_info(),
                ctx.accounts.debt_asset_mint.to_account_info(),
                debt_token_program,
                insurance_request,
                ctx.accounts.debt_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.reserve_vault.reload()?;
            ctx.accounts.insurance_vault.reload()?;
            (
                insurance_balance_before
                    .checked_sub(ctx.accounts.insurance_vault.amount)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
                ctx.accounts
                    .reserve_vault
                    .amount
                    .checked_sub(reserve_balance_before_insurance)
                    .ok_or(ErrorCode::MarketMathOverflow)?,
            )
        } else {
            (0, 0)
        };

        let liquidation_receipt = Liquidation::new(
            args.debt_asset_is_asset0,
            repay_credit,
            insurance_spent,
            insurance_credit,
            args.max_socialized_loss,
        )
        .apply(&mut ctx.accounts.market, &mut ctx.accounts.margin_position)?;
        let collateral_credit = if liquidation_receipt.collateral_seized > 0 {
            let liquidator_collateral_balance_before =
                ctx.accounts.liquidator_collateral_account.amount;
            let collateral_token_program = token_program_for_mint(
                &ctx.accounts.collateral_asset_mint,
                &ctx.accounts.token_program,
                &ctx.accounts.token_2022_program,
            )?;
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.collateral_vault.to_account_info(),
                ctx.accounts.liquidator_collateral_account.to_account_info(),
                ctx.accounts.collateral_asset_mint.to_account_info(),
                collateral_token_program,
                liquidation_receipt.collateral_seized,
                ctx.accounts.collateral_asset_mint.decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.liquidator_collateral_account.reload()?;
            token_account_credit(
                liquidator_collateral_balance_before,
                &ctx.accounts.liquidator_collateral_account,
            )?
        } else {
            0
        };
        require_gte!(
            collateral_credit,
            args.min_collateral_out,
            ErrorCode::SlippageExceeded
        );

        emit_cpi!(MarketLiquidated {
            market: market_key,
            borrower: borrower_key,
            liquidator: liquidator_key,
            debt_asset_mint: debt_asset_mint_key,
            collateral_asset_mint: collateral_asset_mint_key,
            repaid_amount: liquidation_receipt.repaid_amount,
            collateral_seized: liquidation_receipt.collateral_seized,
            insurance_drawn: liquidation_receipt.insurance_drawn,
            socialized_loss: liquidation_receipt.socialized_loss,
            remaining_debt: liquidation_receipt.remaining_debt,
            metadata: MarketEventMetadata::new(liquidator_key, market_key),
        });

        Ok(())
    }
}

fn validate_liquidation_accounts<'info>(
    market: &Account<'info, Market>,
    debt_asset_is_asset0: bool,
    liquidator: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    liquidator_debt_account: &InterfaceAccount<'info, TokenAccount>,
    liquidator_collateral_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (debt_side, collateral_side, insurance_vault_key) = if debt_asset_is_asset0 {
        (
            &market.side0,
            &market.side1,
            market.insurance_reserve.vault0,
        )
    } else {
        (
            &market.side1,
            &market.side0,
            market.insurance_reserve.vault1,
        )
    };
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault_key,
        insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(insurance_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        liquidator_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_debt_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}
