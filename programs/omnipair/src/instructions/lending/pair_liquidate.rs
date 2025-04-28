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
    events::AdjustDebtEvent,
    utils::token::{transfer_from_user_to_pool_vault, transfer_from_pool_vault_to_user},
    generate_gamm_pair_seeds,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct LiquidateArgs {
    pub amount0: u64,
    pub amount1: u64,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
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
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        mut,
        seeds = [
            TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [
            TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
    )]
    pub token1_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1_account: Account<'info, TokenAccount>,

    #[account(address = token0_vault.mint)]
    pub token0_vault_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = token1_vault.mint)]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Liquidate<'info> {
    pub fn validate(&self, args: &LiquidateArgs) -> Result<()> {
        let LiquidateArgs { amount0, amount1 } = args;
        
        require!(*amount0 > 0 || *amount1 > 0, ErrorCode::AmountZero);
        
        if *amount0 > 0 {
            require_gte!(
                self.user_token0_account.amount,
                *amount0,
                ErrorCode::InsufficientAmount0
            );
        }
        
        if *amount1 > 0 {
            require_gte!(
                self.user_token1_account.amount,
                *amount1,
                ErrorCode::InsufficientAmount1
            );
        }
        
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>, args: LiquidateArgs) -> Result<()> {
        let Liquidate {
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

        // Check if position is undercollateralized
        let borrowing_power1 = pair.total_collateral1
            .checked_mul(pair.price1_nad())
            .unwrap()
            .checked_div(NAD)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        
        let borrowing_power0 = pair.total_collateral0
            .checked_mul(pair.price0_nad())
            .unwrap()
            .checked_div(NAD)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        
        require!(
            pair.total_debt0 > borrowing_power1 || pair.total_debt1 > borrowing_power0,
            ErrorCode::InsufficientCollateral
        );

        // Transfer tokens from user to vault
        if args.amount0 > 0 {
            transfer_from_user_to_pool_vault(
                user.to_account_info(),
                user_token0_account.to_account_info(),
                token0_vault.to_account_info(),
                token0_vault_mint.to_account_info(),
                match token0_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount0,
                token0_vault_mint.decimals,
            )?;
            
            // Update debt
            let shares = args.amount0
                .checked_mul(pair.total_debt0_shares)
                .unwrap()
                .checked_div(pair.total_debt0)
                .unwrap();
            pair.total_debt0_shares = pair.total_debt0_shares.checked_sub(shares).unwrap();
            pair.total_debt0 = pair.total_debt0.checked_sub(args.amount0).unwrap();
        }

        if args.amount1 > 0 {
            transfer_from_user_to_pool_vault(
                user.to_account_info(),
                user_token1_account.to_account_info(),
                token1_vault.to_account_info(),
                token1_vault_mint.to_account_info(),
                match token1_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount1,
                token1_vault_mint.decimals,
            )?;
            
            // Update debt
            let shares = args.amount1
                .checked_mul(pair.total_debt1_shares)
                .unwrap()
                .checked_div(pair.total_debt1)
                .unwrap();
            pair.total_debt1_shares = pair.total_debt1_shares.checked_sub(shares).unwrap();
            pair.total_debt1 = pair.total_debt1.checked_sub(args.amount1).unwrap();
        }

        // Transfer collateral to liquidator
        let liquidation_bonus = args.amount0
            .checked_mul(LIQUIDATION_BONUS_BPS)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        
        if liquidation_bonus > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token1_vault.to_account_info(),
                user_token1_account.to_account_info(),
                token1_vault_mint.to_account_info(),
                match token1_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                liquidation_bonus,
                token1_vault_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        // Emit event
        emit!(AdjustDebtEvent {
            user: user.key(),
            amount0: -(args.amount0 as i64),
            amount1: -(args.amount1 as i64),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
