use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::state::pair::Pair;
use crate::state::rate_model::RateModel;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, PAIR_SEED_PREFIX};
use crate::errors::ErrorCode;

#[derive(Accounts)]
pub struct SetPairRateModel<'info> {
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

    /// The new rate model account to assign to this pair.
    pub new_rate_model: Account<'info, RateModel>,

    pub system_program: Program<'info, System>,
}

impl<'info> SetPairRateModel<'info> {
    pub fn handle_set_pair_rate_model(ctx: Context<Self>) -> Result<()> {
        let pair = &mut ctx.accounts.pair;
        let old_rate_model = pair.rate_model;
        let new_rate_model = ctx.accounts.new_rate_model.key();

        pair.rate_model = new_rate_model;

        msg!(
            "Pair rate model updated from {} to {} for pair with tokens ({}, {})",
            old_rate_model,
            new_rate_model,
            pair.token0,
            pair.token1
        );

        Ok(())
    }
}
