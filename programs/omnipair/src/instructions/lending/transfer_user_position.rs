use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, UserPositionTransferredEvent, UserPositionUpdatedEvent},
    state::{pair::Pair, user_position::UserPosition},
    utils::account::get_size_with_discriminator,
};

#[event_cpi]
#[derive(Accounts)]
pub struct TransferUserPosition<'info> {
    #[account(
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref(),
            pair.params_hash.as_ref()
        ],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        constraint = from_position.owner == current_owner.key() @ ErrorCode::InvalidPositionOwner,
        constraint = from_position.pair == pair.key() @ ErrorCode::InvalidPair,
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            current_owner.key().as_ref()
        ],
        bump = from_position.bump
    )]
    pub from_position: Account<'info, UserPosition>,

    #[account(
        init_if_needed,
        payer = current_owner,
        space = get_size_with_discriminator::<UserPosition>(),
        constraint = to_position.owner == Pubkey::default() || to_position.owner == new_owner.key() @ ErrorCode::InvalidPositionOwner,
        constraint = to_position.pair == Pubkey::default() || to_position.pair == pair.key() @ ErrorCode::InvalidPair,
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            new_owner.key().as_ref()
        ],
        bump
    )]
    pub to_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub current_owner: Signer<'info>,
    pub new_owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> TransferUserPosition<'info> {
    pub fn validate_transfer(&self) -> Result<()> {
        require!(
            self.from_position.is_initialized(),
            ErrorCode::UserPositionNotInitialized
        );
        require_keys_neq!(
            self.current_owner.key(),
            self.new_owner.key(),
            ErrorCode::InvalidPositionOwner
        );
        require!(
            self.to_position.collateral0 == 0
                && self.to_position.collateral1 == 0
                && self.to_position.debt0_shares == 0
                && self.to_position.debt1_shares == 0,
            ErrorCode::RecipientPositionNotEmpty
        );

        Ok(())
    }

    pub fn handle_transfer(ctx: Context<Self>) -> Result<()> {
        let from_position_key = ctx.accounts.from_position.key();
        let to_position_key = ctx.accounts.to_position.key();
        let pair_key = ctx.accounts.pair.key();
        let current_owner_key = ctx.accounts.current_owner.key();
        let new_owner_key = ctx.accounts.new_owner.key();
        let from_position_bump = ctx.accounts.from_position.bump;
        let to_position_bump = ctx.bumps.to_position;

        ctx.accounts.to_position.owner = new_owner_key;
        ctx.accounts.to_position.pair = pair_key;
        ctx.accounts.to_position.collateral0_liquidation_cf_bps =
            ctx.accounts.from_position.collateral0_liquidation_cf_bps;
        ctx.accounts.to_position.collateral1_liquidation_cf_bps =
            ctx.accounts.from_position.collateral1_liquidation_cf_bps;
        ctx.accounts.to_position.collateral0 = ctx.accounts.from_position.collateral0;
        ctx.accounts.to_position.collateral1 = ctx.accounts.from_position.collateral1;
        ctx.accounts.to_position.debt0_shares = ctx.accounts.from_position.debt0_shares;
        ctx.accounts.to_position.debt1_shares = ctx.accounts.from_position.debt1_shares;
        ctx.accounts.to_position.bump = to_position_bump;

        ctx.accounts.from_position.owner = Pubkey::default();
        ctx.accounts.from_position.pair = Pubkey::default();
        ctx.accounts.from_position.collateral0_liquidation_cf_bps = 0;
        ctx.accounts.from_position.collateral1_liquidation_cf_bps = 0;
        ctx.accounts.from_position.collateral0 = 0;
        ctx.accounts.from_position.collateral1 = 0;
        ctx.accounts.from_position.debt0_shares = 0;
        ctx.accounts.from_position.debt1_shares = 0;
        ctx.accounts.from_position.bump = from_position_bump;

        emit_cpi!(UserPositionTransferredEvent {
            metadata: EventMetadata::new(current_owner_key, pair_key),
            from_position: from_position_key,
            to_position: to_position_key,
            from_owner: current_owner_key,
            to_owner: new_owner_key,
        });

        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(current_owner_key, pair_key),
            position: from_position_key,
            collateral0: ctx.accounts.from_position.collateral0,
            collateral1: ctx.accounts.from_position.collateral1,
            debt0_shares: ctx.accounts.from_position.debt0_shares,
            debt1_shares: ctx.accounts.from_position.debt1_shares,
            collateral0_max_cf_bps: 0,
            collateral1_max_cf_bps: 0,
            collateral0_liquidation_cf_bps: 0,
            collateral1_liquidation_cf_bps: 0,
        });

        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(new_owner_key, pair_key),
            position: to_position_key,
            collateral0: ctx.accounts.to_position.collateral0,
            collateral1: ctx.accounts.to_position.collateral1,
            debt0_shares: ctx.accounts.to_position.debt0_shares,
            debt1_shares: ctx.accounts.to_position.debt1_shares,
            collateral0_max_cf_bps: ctx
                .accounts
                .to_position
                .get_max_cf_bps_for_debt_token(&ctx.accounts.pair, &ctx.accounts.pair.token1),
            collateral1_max_cf_bps: ctx
                .accounts
                .to_position
                .get_max_cf_bps_for_debt_token(&ctx.accounts.pair, &ctx.accounts.pair.token0),
            collateral0_liquidation_cf_bps: ctx.accounts.to_position.collateral0_liquidation_cf_bps,
            collateral1_liquidation_cf_bps: ctx.accounts.to_position.collateral1_liquidation_cf_bps,
        });

        Ok(())
    }
}
