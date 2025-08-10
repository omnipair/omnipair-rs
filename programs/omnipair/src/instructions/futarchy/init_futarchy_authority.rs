use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::FUTARCHY_AUTHORITY_SEED_PREFIX;
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitFutarchyAuthorityArgs {
    pub authority: Pubkey,
}


#[derive(Accounts)]
pub struct InitFutarchyAuthority<'info> {
    #[account(
        mut,
        address = crate::deployer::ID @ ErrorCode::InvalidDeployer
    )]
    pub deployer: Signer<'info>,

    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<FutarchyAuthority>(),
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitFutarchyAuthority<'info> {
    pub fn handle_init(ctx: Context<Self>, args: InitFutarchyAuthorityArgs) -> Result<()> {
        let futarchy_authority = &mut ctx.accounts.futarchy_authority;
        
        futarchy_authority.set_inner(FutarchyAuthority::initialize(
            args.authority,
            0,
            ctx.bumps.futarchy_authority,
        ));

        Ok(())
    }
}