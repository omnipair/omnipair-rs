use anchor_lang::prelude::*;

use crate::{
    constants::FUTARCHY_AUTHORITY_SEED_PREFIX, errors::ErrorCode, state::FutarchyAuthority,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateFutarchyAuthorityArgs {
    pub new_authority: Pubkey,
}

#[derive(Accounts)]
pub struct UpdateFutarchyAuthority<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    pub system_program: Program<'info, System>,
}

impl<'info> UpdateFutarchyAuthority<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateFutarchyAuthorityArgs) -> Result<()> {
        ctx.accounts.futarchy_authority.authority = args.new_authority;
        Ok(())
    }
}
