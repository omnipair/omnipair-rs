use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeverageDelegationUpdatedEvent},
    state::{Pair, UserLeverageDelegation, UserLeveragePosition},
    utils::account::get_size_with_discriminator,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateLeverageDelegationArgs {
    pub is_debt_token0: bool,
    pub delegated_program: Pubkey,
    pub approved_actions: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateLeverageDelegationArgs {
    pub is_debt_token0: bool,
    pub delegated_program: Pubkey,
    pub approved_actions: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CloseLeverageDelegationArgs {
    pub position: Pubkey,
}

#[derive(Accounts)]
#[instruction(args: CreateLeverageDelegationArgs)]
pub struct CreateLeverageDelegation<'info> {
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
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            owner.key().as_ref(),
            &[args.is_debt_token0 as u8]
        ],
        bump = user_leverage_position.bump,
        constraint = user_leverage_position.owner == owner.key(),
        constraint = user_leverage_position.pair == pair.key(),
        constraint = user_leverage_position.is_debt_token0 == args.is_debt_token0,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        init,
        payer = owner,
        space = get_size_with_discriminator::<UserLeverageDelegation>(),
        seeds = [
            LEVERAGE_DELEGATION_SEED_PREFIX,
            user_leverage_position.key().as_ref(),
        ],
        bump
    )]
    pub user_leverage_delegation: Account<'info, UserLeverageDelegation>,

    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: UpdateLeverageDelegationArgs)]
pub struct UpdateLeverageDelegation<'info> {
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
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            owner.key().as_ref(),
            &[args.is_debt_token0 as u8]
        ],
        bump = user_leverage_position.bump,
        constraint = user_leverage_position.owner == owner.key(),
        constraint = user_leverage_position.pair == pair.key(),
        constraint = user_leverage_position.is_debt_token0 == args.is_debt_token0,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        mut,
        seeds = [
            LEVERAGE_DELEGATION_SEED_PREFIX,
            user_leverage_position.key().as_ref(),
        ],
        bump = user_leverage_delegation.bump,
        constraint = user_leverage_delegation.owner == owner.key(),
        constraint = user_leverage_delegation.pair == pair.key(),
        constraint = user_leverage_delegation.position == user_leverage_position.key(),
        constraint = user_leverage_delegation.is_debt_token0 == args.is_debt_token0,
    )]
    pub user_leverage_delegation: Account<'info, UserLeverageDelegation>,

    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: CloseLeverageDelegationArgs)]
pub struct CloseLeverageDelegation<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [
            LEVERAGE_DELEGATION_SEED_PREFIX,
            args.position.as_ref(),
        ],
        bump = user_leverage_delegation.bump,
        constraint = user_leverage_delegation.owner == owner.key(),
        constraint = user_leverage_delegation.position == args.position,
    )]
    pub user_leverage_delegation: Account<'info, UserLeverageDelegation>,

    #[account(mut)]
    pub owner: Signer<'info>,
}

impl<'info> CreateLeverageDelegation<'info> {
    pub fn handle_create_leverage_delegation(
        ctx: Context<Self>,
        args: CreateLeverageDelegationArgs,
    ) -> Result<()> {
        require!(
            args.delegated_program != Pubkey::default(),
            ErrorCode::InvalidLeverageDelegation
        );

        let delegation = &mut ctx.accounts.user_leverage_delegation;
        delegation.initialize(
            ctx.accounts.owner.key(),
            ctx.accounts.pair.key(),
            ctx.accounts.user_leverage_position.key(),
            args.is_debt_token0,
            args.delegated_program,
            args.approved_actions,
            ctx.bumps.user_leverage_delegation,
        );

        emit!(LeverageDelegationUpdatedEvent {
            metadata: EventMetadata::new(ctx.accounts.owner.key(), ctx.accounts.pair.key()),
            delegation: delegation.key(),
            position: ctx.accounts.user_leverage_position.key(),
            owner: ctx.accounts.owner.key(),
            delegated_program: args.delegated_program,
            approved_actions: args.approved_actions,
        });
        Ok(())
    }
}

impl<'info> UpdateLeverageDelegation<'info> {
    pub fn handle_update_leverage_delegation(
        ctx: Context<Self>,
        args: UpdateLeverageDelegationArgs,
    ) -> Result<()> {
        require!(
            args.delegated_program != Pubkey::default(),
            ErrorCode::InvalidLeverageDelegation
        );

        let delegation = &mut ctx.accounts.user_leverage_delegation;
        delegation.update(args.delegated_program, args.approved_actions);

        emit!(LeverageDelegationUpdatedEvent {
            metadata: EventMetadata::new(ctx.accounts.owner.key(), ctx.accounts.pair.key()),
            delegation: delegation.key(),
            position: ctx.accounts.user_leverage_position.key(),
            owner: ctx.accounts.owner.key(),
            delegated_program: args.delegated_program,
            approved_actions: args.approved_actions,
        });
        Ok(())
    }
}

impl<'info> CloseLeverageDelegation<'info> {
    pub fn handle_close_leverage_delegation(_ctx: Context<Self>, _args: CloseLeverageDelegationArgs) -> Result<()> {
        Ok(())
    }
}
