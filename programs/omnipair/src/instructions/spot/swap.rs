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

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub amount_in: u64,
    pub min_amount_out: u64,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.key().as_ref(), pair.token1.key().as_ref()],
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
    pub fn validate(&self, args: &SwapArgs) -> Result<()> {
        let SwapArgs { amount_in, min_amount_out } = args;

        require!(*amount_in > 0, ErrorCode::AmountZero);
        require!(*min_amount_out > 0, ErrorCode::AmountZero);
        require_gte!(self.user_token_in_account.amount, *amount_in, ErrorCode::InsufficientAmount0In);
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }

    pub fn update_and_validate_swap(&mut self, args: &SwapArgs) -> Result<()> {
        self.update()?;
        self.validate(args)?;
        Ok(())
    }

    pub fn handle_swap(ctx: Context<Self>, args: SwapArgs) -> Result<()> {
        let SwapArgs { amount_in, min_amount_out } = args;
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
            ..        } = ctx.accounts;
        let last_k = (pair.reserve0 as u128).checked_mul(pair.reserve1 as u128).ok_or(ErrorCode::InvariantOverflow)?;
        let is_token0_in = user_token_in_account.mint == pair.token0;

        // amount_in_after_fee = amount_in * (10000 - 30) / 10000 (30bps fee)
        let amount_in_after_fee = (amount_in as u128)
            .checked_mul((BPS_DENOMINATOR - FEE_BPS) as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::FeeMathOverflow)?;

        let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };

        // Δy = (Δx * y) / (x + Δx)
        let denominator = (reserve_in as u128)
            .checked_add(amount_in_after_fee as u128)
            .ok_or(ErrorCode::DenominatorOverflow)?;
        let amount_out = (amount_in_after_fee as u128)
            .checked_mul(reserve_out as u128)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::OutputAmountOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::OutputAmountOverflow)?;

        let new_reserve_in = reserve_in.checked_add(amount_in_after_fee).ok_or(ErrorCode::Overflow)?;
        let new_reserve_out = reserve_out.checked_sub(amount_out).ok_or(ErrorCode::Overflow)?;

        require_gte!(amount_out, min_amount_out, ErrorCode::InsufficientOutputAmount);

        match is_token0_in {
            true => {
                pair.reserve0 = new_reserve_in;
                pair.reserve1 = new_reserve_out;
            },
            false => {
                pair.reserve1 = new_reserve_in;
                pair.reserve0 = new_reserve_out;
            }
        }

        require_gte!((pair.reserve0 as u128).checked_mul(pair.reserve1 as u128).ok_or(ErrorCode::Overflow)?, last_k, ErrorCode::BrokenInvariant);
        
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
        
        let current_time = Clock::get()?.unix_timestamp;

        // Emit event
        emit!(SwapEvent {
            user: user.key(),
            amount0_in: if is_token0_in { amount_in } else { 0 },
            amount1_in: if !is_token0_in { amount_in } else { 0 },
            amount0_out: if is_token0_in { amount_out } else { 0 },
            amount1_out: if !is_token0_in { amount_out } else { 0 },
            timestamp: current_time,
        });
        
        Ok(())
    }
}
