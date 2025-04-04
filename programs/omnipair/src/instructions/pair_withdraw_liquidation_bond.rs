use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ErrorCode;

#[derive(Accounts)]
pub struct WithdrawLiquidationBond<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(mut)]
    pub user_state: Account<'info, UserState>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn withdraw_liquidation_bond(ctx: Context<WithdrawLiquidationBond>) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    let user_state = &mut ctx.accounts.user_state;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Update state
    if current_time > pair.last_update {
        // Update oracles and apply interest
        // ... (same as in swap)
    }
    
    // Verify user has no debt
    require!(
        user_state.debt0_shares == 0 && user_state.debt1_shares == 0,
        ErrorCode::DebtNotZero
    );
    
    // Transfer liquidation bond
    let amount = user_state.liquidation_bond;
    user_state.liquidation_bond = 0;
    
    // Transfer SOL to user
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.pair.key(),
            &ctx.accounts.user.key(),
            amount,
        ),
        &[
            ctx.accounts.pair.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    
    Ok(())
}
