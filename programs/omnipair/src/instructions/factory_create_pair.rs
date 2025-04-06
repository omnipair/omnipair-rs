use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use crate::state::factory::{Factory, PairRegistry};
use crate::state::pair::Pair;
use crate::errors::ErrorCode;
use crate::constants::*;

#[derive(Accounts)]
pub struct CreatePair<'info> {
    #[account(
        mut,
        seeds = [b"factory", factory.owner.as_ref()],
        bump
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        mut,
        seeds = [b"pair_registry", factory.key().as_ref(), &current_registry.registry_index.to_le_bytes()],
        bump
    )]
    pub current_registry: Account<'info, PairRegistry>,
    
    /// CHECK: This account is created on-demand when needed
    #[account(
        init_if_needed,
        payer = payer,
        space = PairRegistry::SIZE,
        seeds = [b"pair_registry", factory.key().as_ref(), &(current_registry.registry_index + 1).to_le_bytes()],
        bump
    )]
    pub next_registry: Account<'info, PairRegistry>,
    
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

pub fn create_pair(ctx: Context<CreatePair>, rate_model: Pubkey) -> Result<()> {
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
    pair.rate_model = rate_model;
    pair.last_update = current_time;
    pair.last_rate0 = MIN_RATE;
    pair.last_rate1 = MIN_RATE;
    
    let factory = &mut ctx.accounts.factory;
    let current_registry = &mut ctx.accounts.current_registry;
    
    // Check if the current registry is full
    if current_registry.pairs.len() >= PairRegistry::MAX_PAIRS_PER_REGISTRY {
        // Initialize the next registry if it's not already initialized
        let next_registry = &mut ctx.accounts.next_registry;
        
        // Only initialize if it's a new account
        if next_registry.factory == Pubkey::default() {
            next_registry.factory = factory.key();
            next_registry.next_registry = None;
            next_registry.registry_index = current_registry.registry_index + 1;
            next_registry.pairs = Vec::with_capacity(PairRegistry::MAX_PAIRS_PER_REGISTRY);
            
            // Link the current registry to the next one
            current_registry.next_registry = Some(next_registry.key());
        }
        
        // Add the pair to the next registry
        next_registry.pairs.push(pair.key());
    } else {
        // Add the pair to the current registry
        current_registry.pairs.push(pair.key());
    }
    
    // Increment the pair count in the factory
    factory.pair_count += 1;
    
    Ok(())
}
