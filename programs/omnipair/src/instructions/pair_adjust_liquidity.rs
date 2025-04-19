use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount},
    associated_token::AssociatedToken,
};
use crate::state::*;
use crate::constants::*;
use crate::utils::calc::*;
use crate::errors::ErrorCode;
use crate::events::*;

#[derive(Debug)]
pub enum LiquidityAction {
    Add(u64),
    Remove(u64),
}

impl LiquidityAction {
    pub fn from_amount(amount: i64) -> Option<Self> {
        match amount {
            x if x > 0 => Some(LiquidityAction::Add(x as u64)),
            x if x < 0 => Some(LiquidityAction::Remove((-x) as u64)),
            _ => None,
        }
    }
}

#[derive(Accounts)]
pub struct AdjustLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            GAMM_PAIR_SEED_PREFIX, 
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        seeds = [
            GAMM_RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            GAMM_RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
    )]
    pub token1_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        associated_token::mint = lp_mint,
        associated_token::authority = user,
        token::token_program = token_program,
    )]
    pub user_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            GAMM_LP_MINT_SEED_PREFIX,
            pair.key().as_ref(),
        ],
        bump,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub rate_model: Account<'info, RateModel>,
}

fn handle_token_transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
    user: &Signer<'info>,
    system_program: &Program<'info, System>,
) -> Result<()> {
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &from.key(),
            &to.key(),
            amount,
        ),
        &[
            from.to_account_info(),
            to.to_account_info(),
            user.to_account_info(),
            system_program.to_account_info(),
        ],
    ).map_err(|e| e.into())
}

impl AdjustLiquidity<'_> {
    pub fn validate(
        ctx: Context<Self>,
        args: AdjustLiquidityArgs,
    ) -> Result<()> {
        let pair = &ctx.accounts.pair;
        
        require!(pair.total_supply > 0, ErrorCode::PairNotInitialized);

        require_gte!(pair.reserve0, args.amount0_in as u64);
        require_gte!(pair.reserve1, args.amount1_in as u64);

        Ok(())
    }


    pub fn adjust_liquidity(
        ctx: Context<Self>,
        args: AdjustLiquidityArgs,
    ) -> Result<()> {
        let pair = &mut ctx.accounts.pair;
        let current_time = Clock::get()?.unix_timestamp;
        
        pair.update(&ctx.accounts.rate_model);

        // Handle token0
        if let Some(action) = LiquidityAction::from_amount(args.amount0_in) {
            match action {
                LiquidityAction::Add(amount) => {
                    handle_token_transfer(
                        &ctx.accounts.user_token0.to_account_info(),
                        &ctx.accounts.token0_reserve_vault.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.reserve0 += amount;
                }
                LiquidityAction::Remove(amount) => {
                    handle_token_transfer(
                        &ctx.accounts.token0_reserve_vault.to_account_info(),
                        &ctx.accounts.user_token0.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.reserve0 -= amount;
                }
            }
        }

        // Handle token1
        if let Some(action) = LiquidityAction::from_amount(args.amount1_in) {
            match action {
                LiquidityAction::Add(amount) => {
                    handle_token_transfer(
                        &ctx.accounts.user_token1.to_account_info(),
                        &ctx.accounts.token1_reserve_vault.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.reserve1 += amount;
                }
                LiquidityAction::Remove(amount) => {
                    handle_token_transfer(
                        &ctx.accounts.token1_reserve_vault.to_account_info(),
                        &ctx.accounts.user_token1.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.reserve1 -= amount;
                }
            }
        }

        // Calculate and handle LP tokens
        let liquidity = if pair.total_supply == 0 {
            let liquidity = sqrt((args.amount0_in as u64) * (args.amount1_in as u64)) - MINIMUM_LIQUIDITY;
            // Mint minimum liquidity to address(0)
            anchor_lang::solana_program::program::invoke(
                &anchor_lang::solana_program::system_instruction::transfer(
                    &pair.key(),
                    &Pubkey::default(),
                    MINIMUM_LIQUIDITY,
                ),
                &[
                    pair.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
            liquidity
        } else {
            min(
                (args.amount0_in as u64) * pair.total_supply / pair.reserve0,
                (args.amount1_in as u64) * pair.total_supply / pair.reserve1,
            )
        };

        // Handle LP tokens
        if let Some(action) = LiquidityAction::from_amount(liquidity as i64) {
            match action {
                LiquidityAction::Add(amount) => {
                    handle_token_transfer(
                        &pair.to_account_info(),
                        &ctx.accounts.user_lp_token.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.total_supply += amount;
                }
                LiquidityAction::Remove(amount) => {
                    handle_token_transfer(
                        &ctx.accounts.user_lp_token.to_account_info(),
                        &pair.to_account_info(),
                        amount,
                        &ctx.accounts.user,
                        &ctx.accounts.system_program,
                    )?;
                    pair.total_supply -= amount;
                }
            }
        }

        // Emit event
        emit!(AdjustLiquidityEvent {
            user: ctx.accounts.user.key(),
            amount0: args.amount0_in as u64,
            amount1: args.amount1_in as u64,
            liquidity: liquidity as u64,
            timestamp: current_time,
        });
        
        Ok(())
    }
}
