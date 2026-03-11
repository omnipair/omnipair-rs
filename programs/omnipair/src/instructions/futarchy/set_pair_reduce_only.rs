use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::state::pair::Pair;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, PAIR_SEED_PREFIX};
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetPairReduceOnlyArgs {
    pub reduce_only: bool,
}

#[derive(Accounts)]
pub struct SetPairReduceOnly<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref(),
            pair.params_hash.as_ref()
        ],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    pub system_program: Program<'info, System>,
}

impl<'info> SetPairReduceOnly<'info> {
    pub fn handle_set_pair_reduce_only(ctx: Context<Self>, args: SetPairReduceOnlyArgs) -> Result<()> {
        let pair = &mut ctx.accounts.pair;
        
        pair.reduce_only = args.reduce_only;

        msg!(
            "Pair reduce-only mode set to: {} for pair with tokens ({}, {})",
            args.reduce_only,
            pair.token0,
            pair.token1
        );

        Ok(())
    }
}
