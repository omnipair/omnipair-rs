use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketStakeUpdated},
    shared::token::transfer_from_user_to_vault,
    state::{Market, StakePosition},
    utils::market_math::active_stake_units,
};

use crate::instructions::common::{
    require_fee_free_claim_mint, token_program_for_mint, validate_stake_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct StakeArgs {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub buffer_shares: u64,
    pub min_active_stake_units: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: StakeArgs)]
pub struct Stake<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub claim_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            STAKE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = stake_position.bump
    )]
    pub stake_position: Box<Account<'info, StakePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Stake<'info> {
    pub fn validate(&self, args: &StakeArgs) -> Result<()> {
        self.market.assert_live()?;
        require!(
            args.claim_amount > 0 && args.buffer_shares > 0,
            ErrorCode::AmountZero
        );
        require_gte!(
            self.owner_claim_account.amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        validate_stake_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.stake_vault,
            &self.owner_claim_account,
        )?;
        require_fee_free_claim_mint(&self.claim_mint)?;
        self.stake_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )?;

        let market_side = self.market.side(args.market_side_index)?;
        require_gte!(
            self.stake_position.available_buffer_shares,
            args.buffer_shares,
            ErrorCode::InsufficientBufferShares
        );
        let next_active_units = active_stake_units(
            self.stake_position
                .staked_claim_amount
                .checked_add(args.claim_amount)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            self.stake_position
                .staked_buffer_shares
                .checked_add(args.buffer_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?,
            market_side.buffer_book.buffer_ratio_bps,
        )?;
        require_gte!(
            next_active_units,
            args.min_active_stake_units,
            ErrorCode::SlippageExceeded
        );
        Ok(())
    }

    pub fn handle_stake(ctx: Context<Self>, args: StakeArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.claim_mint.to_account_info(),
            claim_token_program,
            args.claim_amount,
            ctx.accounts.claim_mint.decimals,
        )?;

        let (active_units, accrued_fee_amount, staked_claim_amount, staked_buffer_shares) = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.carry_forward_unallocated_fee()?;
            ctx.accounts.stake_position.accrue_fees(
                market_side.fee_ledger.fee_growth_index_nad,
                market_side.buffer_book.buffer_ratio_bps,
            )?;
            ctx.accounts
                .stake_position
                .stake(args.claim_amount, args.buffer_shares)?;
            market_side.claim_ledger.staked_claim_supply = market_side
                .claim_ledger
                .staked_claim_supply
                .checked_add(args.claim_amount)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market_side.buffer_book.staked_buffer_shares = market_side
                .buffer_book
                .staked_buffer_shares
                .checked_add(args.buffer_shares)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market_side.carry_forward_unallocated_fee()?;
            (
                ctx.accounts
                    .stake_position
                    .active_stake_units(market_side.buffer_book.buffer_ratio_bps)?,
                ctx.accounts.stake_position.accrued_fee_amount,
                ctx.accounts.stake_position.staked_claim_amount,
                ctx.accounts.stake_position.staked_buffer_shares,
            )
        };

        emit_cpi!(MarketStakeUpdated {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            staked_claim_amount,
            staked_buffer_shares,
            active_stake_units: active_units,
            accrued_fee_amount,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}
