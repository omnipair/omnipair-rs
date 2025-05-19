use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use crate::{
    errors::ErrorCode,
    events::{AdjustCollateralEvent, UserPositionCreatedEvent, UserPositionUpdatedEvent},
    utils::{token::transfer_from_user_to_pool_vault, account::get_size_with_discriminator},
    instructions::lending::common::AdjustPositionArgs,
    state::{user_position::UserPosition, pair::Pair, rate_model::RateModel},
    constants::*,
};

#[derive(Accounts)]
pub struct AddCollateral<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        init_if_needed,
        payer = user,
        space = get_size_with_discriminator::<UserPosition>(),
        constraint = user_position.owner == Pubkey::default() || user_position.owner == user.key(),
        constraint = user_position.pair == Pubkey::default() || user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        mut,
        constraint = collateral_vault.mint == pair.token0 || collateral_vault.mint == pair.token1,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_collateral_token_account.mint == pair.token0 || user_collateral_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_collateral_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = collateral_vault.mint)]
    pub collateral_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddCollateral<'info> {
    pub fn validate_add(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);
        
        require_gte!(
            self.user_collateral_token_account.amount,
            *amount,
            ErrorCode::InsufficientBalanceForCollateral
        );
        
        Ok(())
    }
    
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }

    pub fn update_and_validate_add(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.update()?;
        self.validate_add(args)?;
        Ok(())
    }

    pub fn handle_add_collateral(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
        let AddCollateral { 
            pair, 
            user, 
            collateral_vault,
            collateral_token_mint,
            token_program,
            user_collateral_token_account,
            user_position,
            token_2022_program,
            ..
        } = ctx.accounts;

        if !user_position.is_initialized() {
            user_position.initialize(
                user.key(),
                pair.key(),
                ctx.bumps.user_position,
            )?;

            emit!(UserPositionCreatedEvent {
                user: user.key(),
                pair: pair.key(),
                position: user_position.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        // Transfer tokens from user to collateral vault
        match user_collateral_token_account.mint == pair.token0 {
            true => {
                transfer_from_user_to_pool_vault(
                    user.to_account_info(),
                    user_collateral_token_account.to_account_info(),
                    collateral_vault.to_account_info(),
                    collateral_token_mint.to_account_info(),
                    match collateral_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    collateral_token_mint.decimals,
                )?;
                
                // Update collateral
                pair.total_collateral0 = pair.total_collateral0.checked_add(args.amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_add(args.amount).unwrap();
            },
            false => {
                transfer_from_user_to_pool_vault(
                    user.to_account_info(),
                    user_collateral_token_account.to_account_info(),
                    collateral_vault.to_account_info(),
                    collateral_token_mint.to_account_info(),
                    match collateral_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    collateral_token_mint.decimals,
                )?;
                
                // Update collateral
                pair.total_collateral1 = pair.total_collateral1.checked_add(args.amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_add(args.amount).unwrap();
            }
        }

        // Emit collateral adjustment event
        let (amount0, amount1) = if user_collateral_token_account.mint == pair.token0 {
            (args.amount as i64, 0)
        } else {
            (0, args.amount as i64)
        };
        
        emit!(AdjustCollateralEvent {
            user: user.key(),
            amount0,
            amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Emit position updated event
        emit!(UserPositionUpdatedEvent {
            user: user.key(),
            pair: pair.key(),
            position: user_position.key(),
            collateral0: user_position.collateral0,
            collateral1: user_position.collateral1,
            debt0_shares: user_position.debt0_shares,
            debt1_shares: user_position.debt1_shares,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
