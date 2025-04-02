use anchor_lang::prelude::*;
use crate::state::factory::Factory;
// use crate::state::pair::Pair;
// use crate::errors::AmmError;

#[derive(Accounts)]
pub struct InitializeFactory<'info> {
    #[account(init, payer = payer, space = Factory::SIZE)]
    pub factory: Account<'info, Factory>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_factory(ctx: Context<InitializeFactory>, owner: Pubkey) -> Result<()> {
    let factory = &mut ctx.accounts.factory;
    factory.owner = owner;
    factory.pair_count = 0;
    factory.all_pairs = Vec::new();
    Ok(())
}
