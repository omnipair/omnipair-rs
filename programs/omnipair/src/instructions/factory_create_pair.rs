use anchor_lang::prelude::*;
use crate::state::factory::Factory;
use crate::state::pair::Pair;
use crate::errors::AmmError;

#[derive(Accounts)]
pub struct CreatePair<'info> {
    #[account(mut)]
    pub factory: Account<'info, Factory>,
    /// CHECK: Only storing token mint address.
    pub token0: UncheckedAccount<'info>,
    /// CHECK:
    pub token1: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + Pair::SIZE,
        seeds = [b"pair", token0.key().as_ref(), token1.key().as_ref(), factory.key().as_ref()],
        bump,
    )]
    pub pair: Account<'info, Pair>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create_pair(ctx: Context<CreatePair>, rate_model: Pubkey) -> Result<()> {
    let token0 = ctx.accounts.token0.key();
    let token1 = ctx.accounts.token1.key();
    require!(
        token0 < token1,
        AmmError::InvalidTokenOrder
    );
    {
        let pair = &mut ctx.accounts.pair;
        pair.token0 = token0;
        pair.token1 = token1;
        pair.reserve0 = 0;
        pair.reserve1 = 0;
        pair.last_update = Clock::get()?.unix_timestamp;
        pair.last_price0_ema = 0;
        pair.last_price1_ema = 0;
        pair.rate_model = rate_model;
        pair.last_rate0 = 1e16 as u64;
        pair.last_rate1 = 1e16 as u64;
    }
    let factory = &mut ctx.accounts.factory;
    require!(
        factory.all_pairs.len() < Factory::MAX_PAIRS,
        AmmError::FactoryFull
    );
    factory.all_pairs.push(ctx.accounts.pair.key());
    factory.pair_count = factory.all_pairs.len() as u64;
    Ok(())
}
