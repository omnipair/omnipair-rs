use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, BPS_DENOMINATOR};
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitFutarchyAuthorityArgs {
    pub authority: Pubkey,
    pub swap_bps: u16,
    pub interest_bps: u16,
    pub futarchy_treasury: Pubkey,
    pub futarchy_treasury_bps: u16,
    pub buybacks_vault: Pubkey,
    pub buybacks_vault_bps: u16,
    pub team_treasury: Pubkey,
    pub team_treasury_bps: u16,
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
        let total_percentage = args.futarchy_treasury_bps
            .checked_add(args.buybacks_vault_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_add(args.team_treasury_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?;

        require_eq!(
            total_percentage,
            BPS_DENOMINATOR,
            ErrorCode::InvalidDistribution
        );

        let futarchy_authority = &mut ctx.accounts.futarchy_authority;
        
        let authority = FutarchyAuthority::initialize(
            args.authority,
            args.swap_bps,
            args.interest_bps,
            args.futarchy_treasury,
            args.buybacks_vault,
            args.team_treasury,
            args.futarchy_treasury_bps,
            args.buybacks_vault_bps,
            args.team_treasury_bps,
            ctx.bumps.futarchy_authority,
        )?;
        
        futarchy_authority.set_inner(authority);

        Ok(())
    }
}
