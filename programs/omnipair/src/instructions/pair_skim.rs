use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::constants::*;

#[derive(Accounts)]
pub struct Skim<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub token0: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token1: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn skim(ctx: Context<Skim>) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    
    // Get token balances
    let balance0 = ctx.accounts.token0.amount;
    let balance1 = ctx.accounts.token1.amount;
    
    // Calculate excess tokens
    let excess0 = balance0 - pair.reserve0;
    let excess1 = balance1 - pair.reserve1;
    
    // Transfer excess tokens
    if excess0 > 0 {
        anchor_lang::solana_program::program::invoke(
            &spl_token::instruction::transfer(
                ctx.accounts.token_program.key,
                &ctx.accounts.token0.key(),
                &ctx.accounts.to.key(),
                &ctx.accounts.pair.key(),
                &[],
                excess0,
            )?,
            &[
                ctx.accounts.token0.to_account_info(),
                ctx.accounts.to.to_account_info(),
                ctx.accounts.pair.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;
    }
    
    if excess1 > 0 {
        anchor_lang::solana_program::program::invoke(
            &spl_token::instruction::transfer(
                ctx.accounts.token_program.key,
                &ctx.accounts.token1.key(),
                &ctx.accounts.to.key(),
                &ctx.accounts.pair.key(),
                &[],
                excess1,
            )?,
            &[
                ctx.accounts.token1.to_account_info(),
                ctx.accounts.to.to_account_info(),
                ctx.accounts.pair.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;
    }
    
    // Emit event
    emit!(SkimEvent {
        user: ctx.accounts.user.key(),
        excess0,
        excess1,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
} 