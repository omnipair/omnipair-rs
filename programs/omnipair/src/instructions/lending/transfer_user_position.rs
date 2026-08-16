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
        validate_transfer_state(
            &self.from_position,
            &self.to_position,
            self.current_owner.key(),
            self.new_owner.key(),
        )
    }

    pub fn handle_transfer(ctx: Context<Self>) -> Result<()> {
        let from_position_key = ctx.accounts.from_position.key();
        let to_position_key = ctx.accounts.to_position.key();
        let pair_key = ctx.accounts.pair.key();
        let current_owner_key = ctx.accounts.current_owner.key();
        let new_owner_key = ctx.accounts.new_owner.key();
        let to_position_bump = ctx.bumps.to_position;

        transfer_position_state(
            &mut ctx.accounts.from_position,
            &mut ctx.accounts.to_position,
            pair_key,
            new_owner_key,
            to_position_bump,
        );

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

fn validate_transfer_state(
    from_position: &UserPosition,
    to_position: &UserPosition,
    current_owner: Pubkey,
    new_owner: Pubkey,
) -> Result<()> {
    require!(
        from_position.is_initialized(),
        ErrorCode::UserPositionNotInitialized
    );
    require_keys_neq!(current_owner, new_owner, ErrorCode::InvalidPositionOwner);
    require!(
        to_position.collateral0 == 0
            && to_position.collateral1 == 0
            && to_position.debt0_shares == 0
            && to_position.debt1_shares == 0,
        ErrorCode::RecipientPositionNotEmpty
    );

    Ok(())
}

fn transfer_position_state(
    from_position: &mut UserPosition,
    to_position: &mut UserPosition,
    pair_key: Pubkey,
    new_owner_key: Pubkey,
    to_position_bump: u8,
) {
    let from_position_bump = from_position.bump;

    to_position.owner = new_owner_key;
    to_position.pair = pair_key;
    to_position.collateral0_liquidation_cf_bps = from_position.collateral0_liquidation_cf_bps;
    to_position.collateral1_liquidation_cf_bps = from_position.collateral1_liquidation_cf_bps;
    to_position.collateral0 = from_position.collateral0;
    to_position.collateral1 = from_position.collateral1;
    to_position.debt0_shares = from_position.debt0_shares;
    to_position.debt1_shares = from_position.debt1_shares;
    to_position.bump = to_position_bump;

    from_position.owner = Pubkey::default();
    from_position.pair = Pubkey::default();
    from_position.collateral0_liquidation_cf_bps = 0;
    from_position.collateral1_liquidation_cf_bps = 0;
    from_position.collateral0 = 0;
    from_position.collateral1 = 0;
    from_position.debt0_shares = 0;
    from_position.debt1_shares = 0;
    from_position.bump = from_position_bump;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_position(owner: Pubkey, pair: Pubkey, bump: u8) -> UserPosition {
        UserPosition {
            owner,
            pair,
            collateral0_liquidation_cf_bps: 0,
            collateral1_liquidation_cf_bps: 0,
            collateral0: 0,
            collateral1: 0,
            debt0_shares: 0,
            debt1_shares: 0,
            bump,
        }
    }

    fn populated_position(owner: Pubkey, pair: Pubkey, bump: u8) -> UserPosition {
        UserPosition {
            owner,
            pair,
            collateral0_liquidation_cf_bps: 7_500,
            collateral1_liquidation_cf_bps: 6_900,
            collateral0: 123_456,
            collateral1: 654_321,
            debt0_shares: 111_222_333,
            debt1_shares: 444_555_666,
            bump,
        }
    }

    #[test]
    fn transfer_validation_rejects_uninitialized_source() {
        let current_owner = Pubkey::new_unique();
        let new_owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let from_position = empty_position(Pubkey::default(), Pubkey::default(), 1);
        let to_position = empty_position(new_owner, pair, 2);

        let err = validate_transfer_state(&from_position, &to_position, current_owner, new_owner)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::UserPositionNotInitialized));
    }

    #[test]
    fn transfer_validation_rejects_same_owner() {
        let owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let from_position = populated_position(owner, pair, 1);
        let to_position = empty_position(owner, pair, 2);

        let err = validate_transfer_state(&from_position, &to_position, owner, owner).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidPositionOwner));
    }

    #[test]
    fn transfer_validation_rejects_nonempty_recipient() {
        let current_owner = Pubkey::new_unique();
        let new_owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let from_position = populated_position(current_owner, pair, 1);
        let mut to_position = empty_position(new_owner, pair, 2);
        to_position.debt1_shares = 1;

        let err = validate_transfer_state(&from_position, &to_position, current_owner, new_owner)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::RecipientPositionNotEmpty));
    }

    #[test]
    fn transfer_state_moves_position_and_clears_source_without_changing_totals() {
        let current_owner = Pubkey::new_unique();
        let new_owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let mut from_position = populated_position(current_owner, pair, 9);
        let mut to_position = empty_position(new_owner, pair, 3);
        let total_collateral0_before = from_position.collateral0 + to_position.collateral0;
        let total_collateral1_before = from_position.collateral1 + to_position.collateral1;
        let total_debt0_shares_before = from_position.debt0_shares + to_position.debt0_shares;
        let total_debt1_shares_before = from_position.debt1_shares + to_position.debt1_shares;

        validate_transfer_state(&from_position, &to_position, current_owner, new_owner).unwrap();
        transfer_position_state(&mut from_position, &mut to_position, pair, new_owner, 3);

        assert_eq!(to_position.owner, new_owner);
        assert_eq!(to_position.pair, pair);
        assert_eq!(to_position.collateral0_liquidation_cf_bps, 7_500);
        assert_eq!(to_position.collateral1_liquidation_cf_bps, 6_900);
        assert_eq!(to_position.collateral0, 123_456);
        assert_eq!(to_position.collateral1, 654_321);
        assert_eq!(to_position.debt0_shares, 111_222_333);
        assert_eq!(to_position.debt1_shares, 444_555_666);
        assert_eq!(to_position.bump, 3);

        assert_eq!(from_position.owner, Pubkey::default());
        assert_eq!(from_position.pair, Pubkey::default());
        assert_eq!(from_position.collateral0_liquidation_cf_bps, 0);
        assert_eq!(from_position.collateral1_liquidation_cf_bps, 0);
        assert_eq!(from_position.collateral0, 0);
        assert_eq!(from_position.collateral1, 0);
        assert_eq!(from_position.debt0_shares, 0);
        assert_eq!(from_position.debt1_shares, 0);
        assert_eq!(from_position.bump, 9);

        assert_eq!(
            from_position.collateral0 + to_position.collateral0,
            total_collateral0_before
        );
        assert_eq!(
            from_position.collateral1 + to_position.collateral1,
            total_collateral1_before
        );
        assert_eq!(
            from_position.debt0_shares + to_position.debt0_shares,
            total_debt0_shares_before
        );
        assert_eq!(
            from_position.debt1_shares + to_position.debt1_shares,
            total_debt1_shares_before
        );
    }

    #[test]
    fn transfer_state_can_reuse_a_cleared_source_as_destination() {
        let owner_a = Pubkey::new_unique();
        let owner_b = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let mut owner_a_position = populated_position(owner_a, pair, 8);
        let mut owner_b_position = empty_position(owner_b, pair, 5);

        transfer_position_state(
            &mut owner_a_position,
            &mut owner_b_position,
            pair,
            owner_b,
            5,
        );
        validate_transfer_state(&owner_b_position, &owner_a_position, owner_b, owner_a).unwrap();
        transfer_position_state(
            &mut owner_b_position,
            &mut owner_a_position,
            pair,
            owner_a,
            8,
        );

        assert_eq!(owner_a_position.owner, owner_a);
        assert_eq!(owner_a_position.pair, pair);
        assert_eq!(owner_a_position.collateral0, 123_456);
        assert_eq!(owner_a_position.collateral1, 654_321);
        assert_eq!(owner_a_position.debt0_shares, 111_222_333);
        assert_eq!(owner_a_position.debt1_shares, 444_555_666);
        assert_eq!(owner_a_position.bump, 8);
        assert!(!owner_b_position.is_initialized());
        assert_eq!(owner_b_position.bump, 5);
    }
}
