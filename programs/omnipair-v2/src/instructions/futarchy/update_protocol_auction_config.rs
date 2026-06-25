use anchor_lang::prelude::*;

use crate::{
    constants::FUTARCHY_AUTHORITY_SEED_PREFIX,
    errors::ErrorCode,
    events::ProtocolAuctionConfigUpdated,
    state::{FutarchyAuthority, ProtocolAuctionLane, ProtocolAuctionParams},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateProtocolAuctionConfigArgs {
    pub lane: ProtocolAuctionLane,
    pub accepted_mint: Option<Pubkey>,
    pub params: Option<ProtocolAuctionParams>,
}

#[derive(Accounts)]
pub struct UpdateProtocolAuctionConfig<'info> {
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

impl<'info> UpdateProtocolAuctionConfig<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateProtocolAuctionConfigArgs) -> Result<()> {
        let lane = args.lane;
        let authority = ctx.accounts.futarchy_authority.key();
        let signer = ctx.accounts.authority_signer.key();
        let auction = ctx.accounts.futarchy_authority.auction_config_mut(lane);

        if let Some(accepted_mint) = args.accepted_mint {
            require_keys_neq!(accepted_mint, Pubkey::default(), ErrorCode::InvalidMint);
            auction.accepted_mint = accepted_mint;
        }
        if let Some(params) = args.params {
            params.validate()?;
            auction.params = params;
        }
        auction.validate()?;
        let accepted_mint = auction.accepted_mint;
        let params = auction.params;

        emit!(ProtocolAuctionConfigUpdated {
            authority,
            lane: lane.code(),
            accepted_mint,
            start_multiplier_bps: params.start_multiplier_bps,
            floor_multiplier_bps: params.floor_multiplier_bps,
            duration_slots: params.duration_slots,
            max_reference_age_slots: params.max_reference_age_slots,
            signer,
        });
        Ok(())
    }
}
