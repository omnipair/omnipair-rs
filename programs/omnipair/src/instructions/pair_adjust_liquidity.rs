use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::constants::*;
use crate::utils::math::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Accounts)]
pub struct AdjustLiquidity<'info> {
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

pub fn adjust_liquidity(
    ctx: Context<AdjustLiquidity>,
    amount0_desired: u64,
    amount1_desired: u64,
    amount0_min: u64,
    amount1_min: u64,
) -> Result<()> {
    let pair_key = ctx.accounts.pair.key();
    let pair_info = ctx.accounts.pair.to_account_info();
    let pair_info_clone = pair_info.clone();
    let pair = &mut ctx.accounts.pair;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state
    if current_time > pair.last_update {
        // Update oracles
        let time_elapsed = current_time - pair.last_update;
        if time_elapsed > 0 {
            pair.last_price0_ema = compute_ema(
                pair.last_price0_ema,
                pair.last_update,
                if pair.reserve0 > 0 { pair.reserve1 * SCALE / pair.reserve0 } else { 0 },
                DEFAULT_HALF_LIFE,
                current_time,
            );
            pair.last_price1_ema = compute_ema(
                pair.last_price1_ema,
                pair.last_update,
                if pair.reserve1 > 0 { pair.reserve0 * SCALE / pair.reserve1 } else { 0 },
                DEFAULT_HALF_LIFE,
                current_time,
            );
        }
        pair.last_update = current_time;
    }
    
    // Calculate optimal amounts
    let (amount0, amount1) = if pair.reserve0 == 0 && pair.reserve1 == 0 {
        (amount0_desired, amount1_desired)
    } else {
        let amount1_optimal = (amount0_desired * pair.reserve1) / pair.reserve0;
        if amount1_optimal <= amount1_desired {
            require!(
                amount1_optimal >= amount1_min,
                ErrorCode::InsufficientAmount1
            );
            (amount0_desired, amount1_optimal)
        } else {
            let amount0_optimal = (amount1_desired * pair.reserve0) / pair.reserve1;
            require!(
                amount0_optimal >= amount0_min,
                ErrorCode::InsufficientAmount0
            );
            (amount0_optimal, amount1_desired)
        }
    };
    
    // Transfer tokens
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user_token0.key(),
            &ctx.accounts.token0.key(),
            amount0,
        ),
        &[
            ctx.accounts.user_token0.to_account_info(),
            ctx.accounts.token0.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user_token1.key(),
            &ctx.accounts.token1.key(),
            amount1,
        ),
        &[
            ctx.accounts.user_token1.to_account_info(),
            ctx.accounts.token1.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Calculate liquidity
    let liquidity = if pair.total_supply == 0 {
        let liquidity = sqrt(amount0 * amount1) - MINIMUM_LIQUIDITY;
        // Mint minimum liquidity to address(0)
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &pair_key,
                &Pubkey::default(),
                MINIMUM_LIQUIDITY,
            ),
            &[
                pair_info,
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        liquidity
    } else {
        min(
            amount0 * pair.total_supply / pair.reserve0,
            amount1 * pair.total_supply / pair.reserve1,
        )
    };
    
    // Mint LP tokens
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &pair_key,
            &ctx.accounts.user_lp_token.key(),
            liquidity,
        ),
        &[
            pair_info_clone,
            ctx.accounts.user_lp_token.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    // Update reserves
    pair.reserve0 += amount0;
    pair.reserve1 += amount1;
    pair.total_supply += liquidity;
    
    // Emit event
    emit!(AdjustLiquidityEvent {
        user: ctx.accounts.user.key(),
        amount0,
        amount1,
        liquidity,
        timestamp: current_time,
    });
    
    Ok(())
}
