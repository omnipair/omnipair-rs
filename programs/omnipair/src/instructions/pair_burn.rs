use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Accounts)]
pub struct Burn<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub token0: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token1: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_token0: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token1: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_lp_token: Account<'info, TokenAccount>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn burn(
    ctx: Context<Burn>,
    liquidity: u64,
    min_amount0: u64,
    min_amount1: u64,
) -> Result<()> {
    let pair_key = ctx.accounts.pair.key();
    let pair_info = ctx.accounts.pair.to_account_info();
    let pair = &mut ctx.accounts.pair;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state
    if current_time > pair.last_update {
        // Update oracles and apply interest
        // ... (same as in swap)
    }
    
    // Calculate amounts to withdraw
    let amount0 = liquidity * pair.reserve0 / pair.total_supply;
    let amount1 = liquidity * pair.reserve1 / pair.total_supply;
    
    require!(
        amount0 >= min_amount0 && amount1 >= min_amount1,
        ErrorCode::InsufficientOutputAmount
    );
    
    // Burn LP tokens
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user_lp_token.key(),
            &pair_key,
            liquidity,
        ),
        &[
            ctx.accounts.user_lp_token.to_account_info(),
            pair_info,
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Transfer tokens back to user
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.token0.key(),
            &ctx.accounts.user_token0.key(),
            amount0,
        ),
        &[
            ctx.accounts.token0.to_account_info(),
            ctx.accounts.user_token0.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.token1.key(),
            &ctx.accounts.user_token1.key(),
            amount1,
        ),
        &[
            ctx.accounts.token1.to_account_info(),
            ctx.accounts.user_token1.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Update reserves
    pair.reserve0 -= amount0;
    pair.reserve1 -= amount1;
    pair.total_supply -= liquidity;
    
    // Emit event
    emit!(BurnEvent {
        user: ctx.accounts.user.key(),
        amount0,
        amount1,
        liquidity,
        timestamp: current_time,
    });
    
    Ok(())
}
