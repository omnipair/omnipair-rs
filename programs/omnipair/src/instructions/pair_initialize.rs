use anchor_lang::prelude::*;
use anchor_spl::token::{Token, Mint};
use crate::state::pair::Pair;
use crate::state::rate_model::RateModel;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;

#[derive(Accounts)]
pub struct InitializePair<'info> {
    pub token0: Account<'info, Mint>,
    pub token1: Account<'info, Mint>,
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        init,
        payer = payer,
        space = get_size_with_discriminator::<Pair>(),
        seeds = [GAMM_PAIR_SEED_PREFIX, token0.key().as_ref(), token1.key().as_ref()],
        bump
    )]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_pair(ctx: Context<InitializePair>) -> Result<()> {
    let token0 = ctx.accounts.token0.key();
    let token1 = ctx.accounts.token1.key();
    
    // Enforce token0 < token1 to ensure unique pair addresses.
    // This prevents the same token pair from having two valid addresses (A,B) and (B,A).
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
    pair.rate_model = ctx.accounts.rate_model.key();
    
    Ok(())
} 