use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, FUTARCHY_AUTHORITY_SEED_PREFIX},
    errors::ErrorCode,
    events::ProtocolAuctionRecipientsUpdated,
    state::{FutarchyAuthority, ProtocolAuctionLane},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateProtocolAuctionRecipientsArgs {
    pub lane: ProtocolAuctionLane,
    pub treasury: Option<Pubkey>,
    pub staking_vault: Option<Pubkey>,
    pub treasury_bps: Option<u16>,
    pub staking_vault_bps: Option<u16>,
}

#[derive(Accounts)]
pub struct UpdateProtocolAuctionRecipients<'info> {
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

impl<'info> UpdateProtocolAuctionRecipients<'info> {
    pub fn handle_update(
        ctx: Context<Self>,
        args: UpdateProtocolAuctionRecipientsArgs,
    ) -> Result<()> {
        let lane = args.lane;
        let authority = ctx.accounts.futarchy_authority.key();
        let signer = ctx.accounts.authority_signer.key();
        let auction = ctx.accounts.futarchy_authority.auction_config_mut(lane);

        if let Some(treasury) = args.treasury {
            auction.recipients.treasury = treasury;
        }
        if let Some(staking_vault) = args.staking_vault {
            auction.recipients.staking_vault = staking_vault;
        }
        if let Some(treasury_bps) = args.treasury_bps {
            require_gte!(
                BPS_DENOMINATOR,
                treasury_bps,
                ErrorCode::InvalidDistribution
            );
            auction.recipients.treasury_bps = treasury_bps;
        }
        if let Some(staking_vault_bps) = args.staking_vault_bps {
            require_gte!(
                BPS_DENOMINATOR,
                staking_vault_bps,
                ErrorCode::InvalidDistribution
            );
            auction.recipients.staking_vault_bps = staking_vault_bps;
        }
        require!(
            auction.recipients.is_valid(),
            ErrorCode::InvalidDistribution
        );
        let recipients = auction.recipients;

        emit!(ProtocolAuctionRecipientsUpdated {
            authority,
            lane: lane.code(),
            treasury: recipients.treasury,
            staking_vault: recipients.staking_vault,
            treasury_bps: recipients.treasury_bps,
            staking_vault_bps: recipients.staking_vault_bps,
            signer,
        });
        Ok(())
    }
}
