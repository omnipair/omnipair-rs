use anchor_lang::prelude::*;
use crate::state::futarchy_authority::{FutarchyAuthority, RevenueDistribution};
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, BPS_DENOMINATOR};
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateProtocolRevenueArgs {
    pub swap_bps: Option<u16>,
    pub interest_bps: Option<u16>,
    pub revenue_distribution: Option<RevenueDistribution>,
}


#[derive(Accounts)]
pub struct UpdateProtocolRevenue<'info> {
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

impl<'info> UpdateProtocolRevenue<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateProtocolRevenueArgs) -> Result<()> {
        let futarchy_authority = &mut ctx.accounts.futarchy_authority;

        // Update revenue share if provided
        if let Some(swap_bps) = args.swap_bps {
            require_gte!(BPS_DENOMINATOR, swap_bps, ErrorCode::InvalidSwapFeeBps);
            futarchy_authority.revenue_share.swap_bps = swap_bps;
        }
        if let Some(interest_bps) = args.interest_bps {
            require_gte!(BPS_DENOMINATOR, interest_bps, ErrorCode::InvalidInterestFeeBps); // InvalidInterestFeeBps will be merged in bf38eb3c4fad1bcd1afbb3c12d15a072b3f8860f
            futarchy_authority.revenue_share.interest_bps = interest_bps;
        }

        // Update revenue distribution if provided
        if let Some(revenue_distribution) = args.revenue_distribution {
            require!(revenue_distribution.is_valid(), ErrorCode::InvalidDistribution);
            futarchy_authority.revenue_distribution = revenue_distribution;
        }

        Ok(())
    }
}
