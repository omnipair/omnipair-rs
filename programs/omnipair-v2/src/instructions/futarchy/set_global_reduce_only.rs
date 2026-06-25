use anchor_lang::prelude::*;

use crate::{
    constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, REDUCE_ONLY_EMERGENCY_AUTHORITY},
    errors::ErrorCode,
    state::FutarchyAuthority,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetGlobalReduceOnlyArgs {
    pub reduce_only: bool,
}

#[derive(Accounts)]
pub struct SetGlobalReduceOnly<'info> {
    #[account(
        mut,
        address = REDUCE_ONLY_EMERGENCY_AUTHORITY @ ErrorCode::InvalidReduceOnlyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,
}

impl<'info> SetGlobalReduceOnly<'info> {
    pub fn handle_set_global_reduce_only(
        ctx: Context<Self>,
        args: SetGlobalReduceOnlyArgs,
    ) -> Result<()> {
        ctx.accounts.futarchy_authority.global_reduce_only = args.reduce_only;
        msg!("Global reduce-only mode set to: {}", args.reduce_only);
        Ok(())
    }
}
