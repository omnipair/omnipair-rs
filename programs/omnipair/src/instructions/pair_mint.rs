use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::constants::*;
use crate::utils::math::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Accounts)]
pub struct MintLiquidity<'info> {
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

pub fn mint(
    ctx: Context<MintLiquidity>,
    amount0: u64,
    amount1: u64,
    min_liquidity: u64,
) -> Result<()> {
    let pair_key = ctx.accounts.pair.key();
    let pair_info = ctx.accounts.pair.to_account_info();
    let pair_info_clone = pair_info.clone();
    let pair = &mut ctx.accounts.pair;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state
    if current_time > pair.last_update {
        // Update oracles and apply interest
        // ... (same as in swap)
    }
    
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
    
    require!(
        liquidity > min_liquidity,
        ErrorCode::InsufficientLiquidity
    );
    
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
    emit!(MintEvent {
        user: ctx.accounts.user.key(),
        amount0,
        amount1,
        liquidity,
        timestamp: current_time,
    });
    
    Ok(())
}
