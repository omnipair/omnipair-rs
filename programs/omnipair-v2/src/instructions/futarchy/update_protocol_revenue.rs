use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, FUTARCHY_AUTHORITY_SEED_PREFIX},
    errors::ErrorCode,
    state::{FutarchyAuthority, RevenueDistribution},
};

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
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub system_program: Program<'info, System>,
}

impl<'info> UpdateProtocolRevenue<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateProtocolRevenueArgs) -> Result<()> {
        if let Some(swap_bps) = args.swap_bps {
            require_gte!(BPS_DENOMINATOR, swap_bps, ErrorCode::InvalidSwapFeeBps);
            ctx.accounts.futarchy_authority.revenue_share.swap_bps = swap_bps;
        }
        if let Some(interest_bps) = args.interest_bps {
            require_gte!(
                BPS_DENOMINATOR,
                interest_bps,
                ErrorCode::InvalidInterestFeeBps
            );
            ctx.accounts.futarchy_authority.revenue_share.interest_bps = interest_bps;
        }
        if let Some(revenue_distribution) = args.revenue_distribution {
            require!(
                revenue_distribution.is_valid(),
                ErrorCode::InvalidDistribution
            );
            ctx.accounts.futarchy_authority.revenue_distribution = revenue_distribution;
        }
        Ok(())
    }
}
