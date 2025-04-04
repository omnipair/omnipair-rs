use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;

#[derive(Accounts)]
pub struct UpdateOracle<'info> {
    #[account(
        mut,
        constraint = pair.initialized @ ErrorCode::PairNotInitialized
    )]
    pub pair: Account<'info, Pair>,
}

pub fn update_oracle(ctx: Context<UpdateOracle>) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update oracles
    let time_elapsed = current_time - pair.last_update;
    if time_elapsed > 0 {
        pair.price0_cumulative_last += pair.price0_last * time_elapsed as u128;
        pair.price1_cumulative_last += pair.price1_last * time_elapsed as u128;
    }
    pair.last_update = current_time;
    
    // Emit event
    emit!(UpdateOracleEvent {
        price0_cumulative: pair.price0_cumulative_last,
        price1_cumulative: pair.price1_cumulative_last,
        timestamp: current_time,
    });
    
    Ok(())
} 