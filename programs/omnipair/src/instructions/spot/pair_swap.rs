use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount},
    token_interface::{Mint, Token2022},
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    events::*,
    utils::token::{transfer_from_user_to_pool_vault, transfer_from_pool_vault_to_user},
    generate_gamm_pair_seeds,
};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, token_in_mint.key().as_ref(), token_out_mint.key().as_ref()],
        bump
    )]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        constraint = token_in_vault.mint == pair.token0 || token_in_vault.mint == pair.token1,
    )]
    pub token_in_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token_out_vault.mint == pair.token0 || token_out_vault.mint == pair.token1,
    )]
    pub token_out_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_token_in_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_out_account: Account<'info, TokenAccount>,

    #[account(address = token_in_vault.mint)]
    pub token_in_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(address = token_out_vault.mint)]
    pub token_out_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Swap<'info> {
    pub fn validate(&self, amount_in: u64, min_amount_out: u64) -> Result<()> {
        require!(amount_in > 0, ErrorCode::AmountZero);
        require!(min_amount_out > 0, ErrorCode::AmountZero);
        require_gte!(self.user_token_in_account.amount, amount_in, ErrorCode::InsufficientAmount0In);
        Ok(())
    }

    pub fn handle_swap(ctx: Context<Self>, amount_in: u64, min_amount_out: u64) -> Result<()> {
        let Swap {
            pair,
            token_in_vault,
            token_out_vault,
            user_token_in_account,
            user_token_out_account,
            token_in_mint,
            token_out_mint,
            token_program,
            token_2022_program,
            user,
            ..
        } = ctx.accounts;

        // Update state 
        let current_time = Clock::get()?.unix_timestamp;
        if current_time > pair.last_update {
            let time_elapsed = current_time - pair.last_update;
            if time_elapsed > 0 {
                pair.price0_cumulative_last = pair.price0_cumulative_last
                    .checked_add((pair.price0_last as u128) * (time_elapsed as u128))
                    .unwrap();
                pair.price1_cumulative_last = pair.price1_cumulative_last
                    .checked_add((pair.price1_last as u128) * (time_elapsed as u128))
                    .unwrap();
            }
            pair.last_update = current_time;
        }
        
        // Calculate output amount
        let amount_out = match user_token_in_account.mint == pair.token0 {
            true => {
                // Swap token0 for token1
                let amount_out = (amount_in as u128)
                    .checked_mul(pair.reserve1 as u128)
                    .unwrap()
                    .checked_div((pair.reserve0 + amount_in) as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
                
                require!(
                    amount_out >= min_amount_out,
                    ErrorCode::InsufficientOutputAmount
                );
                
                // Update reserves
                pair.reserve0 = pair.reserve0.checked_add(amount_in).unwrap();
                pair.reserve1 = pair.reserve1.checked_sub(amount_out).unwrap();
                
                amount_out
            },
            false => {
                // Swap token1 for token0
                let amount_out = (amount_in as u128)
                    .checked_mul(pair.reserve0 as u128)
                    .unwrap()
                    .checked_div((pair.reserve1 + amount_in) as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
                
                require!(
                    amount_out >= min_amount_out,
                    ErrorCode::InsufficientOutputAmount
                );
                
                // Update reserves
                pair.reserve1 = pair.reserve1.checked_add(amount_in).unwrap();
                pair.reserve0 = pair.reserve0.checked_sub(amount_out).unwrap();
                
                amount_out
            }
        };
        
        // Transfer tokens
        transfer_from_user_to_pool_vault(
            user.to_account_info(),
            user_token_in_account.to_account_info(),
            token_in_vault.to_account_info(),
            token_in_mint.to_account_info(),
            match token_in_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount_in,
            token_in_mint.decimals,
        )?;

        transfer_from_pool_vault_to_user(
            pair.to_account_info(),
            token_out_vault.to_account_info(),
            user_token_out_account.to_account_info(),
            token_out_mint.to_account_info(),
            match token_out_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount_out,
            token_out_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // Update prices
        pair.price0_last = (pair.reserve1 as u128)
            .checked_mul(SCALE as u128)
            .unwrap()
            .checked_div(pair.reserve0 as u128)
            .unwrap()
            .try_into()
            .unwrap();
        pair.price1_last = (pair.reserve0 as u128)
            .checked_mul(SCALE as u128)
            .unwrap()
            .checked_div(pair.reserve1 as u128)
            .unwrap()
            .try_into()
            .unwrap();
        
        // Emit event
        emit!(SwapEvent {
            user: user.key(),
            amount0_in: if user_token_in_account.mint == pair.token0 { amount_in } else { 0 },
            amount1_in: if user_token_in_account.mint == pair.token1 { amount_in } else { 0 },
            amount0_out: if user_token_out_account.mint == pair.token0 { amount_out } else { 0 },
            amount1_out: if user_token_out_account.mint == pair.token1 { amount_out } else { 0 },
            timestamp: current_time,
        });
        
        Ok(())
    }
}
