use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    constants::*,
    errors::ErrorCode,
    events::AdjustCollateralEvent,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{AdjustCollateral, AdjustCollateralArgs},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemoveCollateralArgs {
    pub amount0: u64,
    pub amount1: u64,
}

#[derive(Accounts)]
pub struct RemoveCollateral<'info> {
    #[account(
        mut,
        seeds = [
            GAMM_PAIR_SEED_PREFIX, 
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
    )]
    pub token1_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = token0_vault.mint)]
    pub token0_vault_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = token1_vault.mint)]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> AdjustCollateral<'info> {
    pub fn validate_remove(&self, args: &AdjustCollateralArgs) -> Result<()> {
        let AdjustCollateralArgs { amount0, amount1 } = args;
        
        require!(*amount0 > 0 || *amount1 > 0, ErrorCode::AmountZero);
        
        if *amount0 > 0 {
            require_gte!(
                self.pair.total_collateral0,
                *amount0,
                ErrorCode::InsufficientCollateral0
            );
        }
        
        if *amount1 > 0 {
            require_gte!(
                self.pair.total_collateral1,
                *amount1,
                ErrorCode::InsufficientCollateral1
            );
        }
        
        Ok(())
    }

    pub fn validate_remove_and_update(&mut self, args: &AdjustCollateralArgs) -> Result<()> {
        self.validate_remove(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_remove_collateral(ctx: Context<Self>, args: AdjustCollateralArgs) -> Result<()> {
        let AdjustCollateral {
            pair,
            token0_vault,
            token1_vault,
            user_token0_account,
            user_token1_account,
            token0_vault_mint,
            token1_vault_mint,
            token_program,
            token_2022_program,
            user,
            ..
        } = ctx.accounts;

        // Update pair state
        pair.update(&ctx.accounts.rate_model)?;

        // Transfer tokens from vault to user
        if args.amount0 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token0_vault.to_account_info(),
                user_token0_account.to_account_info(),
                token0_vault_mint.to_account_info(),
                match token0_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount0,
                token0_vault_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
            
            // Update collateral
            pair.total_collateral0 = pair.total_collateral0.checked_sub(args.amount0).unwrap();
        }

        if args.amount1 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token1_vault.to_account_info(),
                user_token1_account.to_account_info(),
                token1_vault_mint.to_account_info(),
                match token1_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount1,
                token1_vault_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
            
            // Update collateral
            pair.total_collateral1 = pair.total_collateral1.checked_sub(args.amount1).unwrap();
        }

        // Emit event
        emit!(AdjustCollateralEvent {
            user: user.key(),
            amount0: -(args.amount0 as i64),
            amount1: -(args.amount1 as i64),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
