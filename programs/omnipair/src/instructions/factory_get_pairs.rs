use anchor_lang::prelude::*;
use crate::state::factory::{Factory, PairRegistry};

#[derive(Accounts)]
#[instruction(registry_index: u32)]
pub struct GetPairs<'info> {
    #[account(
        seeds = [b"factory", factory.owner.as_ref()],
        bump
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        seeds = [b"pair_registry", factory.key().as_ref(), &registry_index.to_le_bytes()],
        bump
    )]
    pub registry: Account<'info, PairRegistry>,
}

pub fn get_pairs(ctx: Context<GetPairs>) -> Result<Vec<Pubkey>> {
    // Return the pairs from the specified registry
    Ok(ctx.accounts.registry.pairs.clone())
} 