use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use crate::state::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Accounts)]
pub struct Sync<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        constraint = token0.key() == pair.token0 @ ErrorCode::InvalidTokenAccount
    )]
    pub token0: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token1.key() == pair.token1 @ ErrorCode::InvalidTokenAccount
    )]
    pub token1: Account<'info, TokenAccount>,
}

pub fn sync(ctx: Context<Sync>) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    
    // Get token balances
    let balance0 = ctx.accounts.token0.amount;
    let balance1 = ctx.accounts.token1.amount;
    
    // Update reserves
    pair.reserve0 = balance0;
    pair.reserve1 = balance1;
    
    // Update prices
    pair.price0_last = (balance1 * PRICE_PRECISION) / balance0;
    pair.price1_last = (balance0 * PRICE_PRECISION) / balance1;
    
    // Emit event
    emit!(SyncEvent {
        reserve0: balance0,
        reserve1: balance1,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
} 