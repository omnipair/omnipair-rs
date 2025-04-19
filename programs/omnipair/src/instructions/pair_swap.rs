use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub token_in: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_out: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_token_in: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_out: Account<'info, TokenAccount>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn swap(
    ctx: Context<Swap>,
    amount_in: u64,
    min_amount_out: u64,
) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    let token0 = pair.token0;
    let token1 = pair.token1;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state 
    if current_time > pair.last_update {
        // Update oracles
        let time_elapsed = current_time - pair.last_update;
        if time_elapsed > 0 {
            pair.price0_cumulative_last += (pair.price0_last as u128) * (time_elapsed as u128);
            pair.price1_cumulative_last += (pair.price1_last as u128) * (time_elapsed as u128);
        }
        pair.last_update = current_time;
    }
    
    // Calculate output amount
    let amount_out = if ctx.accounts.token_in.key() == token0 {
        // Swap token0 for token1
        let amount_out = (amount_in * pair.reserve1) / (pair.reserve0 + amount_in);
        require!(
            amount_out >= min_amount_out,
            ErrorCode::InsufficientOutputAmount
        );
        
        // Update reserves
        pair.reserve0 += amount_in;
        pair.reserve1 -= amount_out;
        
        amount_out
    } else {
        // Swap token1 for token0
        let amount_out = (amount_in * pair.reserve0) / (pair.reserve1 + amount_in);
        require!(
            amount_out >= min_amount_out,
            ErrorCode::InsufficientOutputAmount
        );
        
        // Update reserves
        pair.reserve1 += amount_in;
        pair.reserve0 -= amount_out;
        
        amount_out
    };
    
    // Transfer tokens
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user_token_in.key(),
            &ctx.accounts.token_in.key(),
            amount_in,
        ),
        &[
            ctx.accounts.user_token_in.to_account_info(),
            ctx.accounts.token_in.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.token_out.key(),
            &ctx.accounts.user_token_out.key(),
            amount_out,
        ),
        &[
            ctx.accounts.token_out.to_account_info(),
            ctx.accounts.user_token_out.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Update prices
    pair.price0_last = (pair.reserve1 * SCALE) / pair.reserve0;
    pair.price1_last = (pair.reserve0 * SCALE) / pair.reserve1;
    
    // Emit event
    emit!(SwapEvent {
        user: ctx.accounts.user.key(),
        amount0_in: if ctx.accounts.token_in.key() == token0 { amount_in } else { 0 },
        amount1_in: if ctx.accounts.token_in.key() == token1 { amount_in } else { 0 },
        amount0_out: if ctx.accounts.token_out.key() == token0 { amount_out } else { 0 },
        amount1_out: if ctx.accounts.token_out.key() == token1 { amount_out } else { 0 },
        timestamp: current_time,
    });
    
    Ok(())
}
