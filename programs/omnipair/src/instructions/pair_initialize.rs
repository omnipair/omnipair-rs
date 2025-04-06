use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use crate::state::pair::Pair;
use crate::errors::ErrorCode;
use crate::constants::*;

#[derive(Accounts)]
pub struct InitializePair<'info> {
    /// CHECK: Only storing token mint address
    pub token0: UncheckedAccount<'info>,
    /// CHECK: Only storing token mint address
    pub token1: UncheckedAccount<'info>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + Pair::SIZE,
        seeds = [b"pair", token0.key().as_ref(), token1.key().as_ref()],
        bump,
    )]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_pair(ctx: Context<InitializePair>, rate_model: Pubkey) -> Result<()> {
    let token0 = ctx.accounts.token0.key();
    let token1 = ctx.accounts.token1.key();
    
    require!(
        token0 < token1,
        ErrorCode::InvalidTokenOrder
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    
    let pair = &mut ctx.accounts.pair;
    pair.token0 = token0;
    pair.token1 = token1;
    pair.last_update = current_time;
    pair.last_rate0 = MIN_RATE;
    pair.last_rate1 = MIN_RATE;
    pair.rate_model = rate_model;
    
    Ok(())
} 