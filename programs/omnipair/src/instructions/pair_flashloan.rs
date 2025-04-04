use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::errors::ErrorCode;

#[derive(Accounts)]
pub struct Flashloan<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub token0: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token1: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub receiver: Account<'info, TokenAccount>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn flashloan(
    ctx: Context<Flashloan>,
    amount0: u64,
    amount1: u64,
    data: Vec<u8>,
) -> Result<()> {
    let pair_info = ctx.accounts.pair.to_account_info();
    let pair_info_clone = pair_info.clone();
    let pair = &mut ctx.accounts.pair;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state
    if current_time > pair.last_update {
        // Update oracles and apply interest
        // ... (same as in swap)
    }
    
    // Store initial balances
    let balance0 = ctx.accounts.token0.amount;
    let balance1 = ctx.accounts.token1.amount;
    
    // Transfer tokens to receiver
    if amount0 > 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.token0.key(),
                &ctx.accounts.receiver.key(),
                amount0,
            ),
            &[
                ctx.accounts.token0.to_account_info(),
                ctx.accounts.receiver.to_account_info(),
                pair_info,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }
    
    if amount1 > 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.token1.key(),
                &ctx.accounts.receiver.key(),
                amount1,
            ),
            &[
                ctx.accounts.token1.to_account_info(),
                ctx.accounts.receiver.to_account_info(),
                pair_info_clone,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }
    
    // Call onFlashloan on the receiver
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = anchor_spl::token::Transfer {
        from: ctx.accounts.receiver.to_account_info(),
        to: ctx.accounts.token0.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.receiver.key(),
            &ctx.accounts.token0.key(),
            amount0,
        ),
        &[
            ctx.accounts.receiver.to_account_info(),
            ctx.accounts.token0.to_account_info(),
            ctx.accounts.pair.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.receiver.key(),
            &ctx.accounts.token1.key(),
            amount1,
        ),
        &[
            ctx.accounts.receiver.to_account_info(),
            ctx.accounts.token1.to_account_info(),
            ctx.accounts.pair.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Verify balances are restored
    require!(
        ctx.accounts.token0.amount == balance0 && 
        ctx.accounts.token1.amount == balance1,
        ErrorCode::FlashloanNotRepaid
    );
    
    Ok(())
} 