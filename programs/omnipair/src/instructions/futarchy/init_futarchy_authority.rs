use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, BPS_DENOMINATOR};
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitFutarchyAuthorityArgs {
    pub authority: Pubkey,
    pub recipient1: Pubkey,
    pub recipient1_percentage_bps: u16,
    pub recipient2: Pubkey,
    pub recipient2_percentage_bps: u16,
    pub recipient3: Pubkey,
    pub recipient3_percentage_bps: u16,
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
        // Validate percentages sum to 100%
        let total_percentage = args.recipient1_percentage_bps
            .checked_add(args.recipient2_percentage_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_add(args.recipient3_percentage_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?;

        require_eq!(
            total_percentage,
            BPS_DENOMINATOR,
            ErrorCode::InvalidDistribution
        );

        let futarchy_authority = &mut ctx.accounts.futarchy_authority;
        
        futarchy_authority.set_inner(FutarchyAuthority::initialize(
            args.authority,
            0,
            args.recipient1,
            args.recipient1_percentage_bps,
            args.recipient2,
            args.recipient2_percentage_bps,
            args.recipient3,
            args.recipient3_percentage_bps,
            ctx.bumps.futarchy_authority,
        ));

        Ok(())
    }
}