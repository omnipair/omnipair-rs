use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::FUTARCHY_AUTHORITY_SEED_PREFIX;
use crate::errors::ErrorCode;

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
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub system_program: Program<'info, System>,
}

impl<'info> UpdateFutarchyAuthority<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateFutarchyAuthorityArgs) -> Result<()> {
        let futarchy_authority = &mut ctx.accounts.futarchy_authority;
        
        futarchy_authority.authority = args.new_authority;

        Ok(())
    }
}