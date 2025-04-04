use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::constants::*;
use crate::errors::ErrorCode;

#[derive(Accounts)]
pub struct AdjustCollateral<'info> {
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
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn adjust_collateral(
    ctx: Context<AdjustCollateral>,
    amount0: i64,
    amount1: i64,
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
    
    // Handle token0
    if amount0 > 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.user_token0.key(),
                &ctx.accounts.token0.key(),
                amount0 as u64,
            ),
            &[
                ctx.accounts.user_token0.to_account_info(),
                ctx.accounts.token0.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        pair.total_collateral0 += amount0 as u64;
    } else if amount0 < 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.token0.key(),
                &ctx.accounts.user_token0.key(),
                (-amount0) as u64,
            ),
            &[
                ctx.accounts.token0.to_account_info(),
                ctx.accounts.user_token0.to_account_info(),
                pair_info,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        pair.total_collateral0 -= (-amount0) as u64;
        
        // Check borrowing power
        let debt1 = pair.total_debt1;
        if debt1 > 0 {
            let borrowing_power0 = pair.total_collateral0 * pair.last_price0_ema * CF_BPS / SCALE / 10000;
            require!(debt1 <= borrowing_power0, ErrorCode::BorrowingPowerExceeded);
        }
    }
    
    // Handle token1
    if amount1 > 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.token1.key(),
                &ctx.accounts.user_token1.key(),
                amount1 as u64,
            ),
            &[
                ctx.accounts.token1.to_account_info(),
                ctx.accounts.user_token1.to_account_info(),
                pair_info_clone,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        pair.total_collateral1 += amount1 as u64;
    } else if amount1 < 0 {
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.token1.key(),
                &ctx.accounts.user_token1.key(),
                (-amount1) as u64,
            ),
            &[
                ctx.accounts.token1.to_account_info(),
                ctx.accounts.user_token1.to_account_info(),
                pair_info_clone,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        pair.total_collateral1 -= (-amount1) as u64;
        
        // Check borrowing power
        let debt0 = pair.total_debt0;
        if debt0 > 0 {
            let borrowing_power1 = pair.total_collateral1 * pair.last_price1_ema * CF_BPS / SCALE / 10000;
            require!(debt0 <= borrowing_power1, ErrorCode::BorrowingPowerExceeded);
        }
    }
    
    Ok(())
}
