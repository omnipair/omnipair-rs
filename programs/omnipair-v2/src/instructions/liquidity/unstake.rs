use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketStakeUpdated},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{Market, MarketAsset, StakePosition},
    transitions::staking::Unstake as UnstakeTransition,
};

use crate::instructions::common::{
    require_fee_free_claim_token_mint, token_program_for_mint, validate_stake_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UnstakeArgs {
    pub market_asset: MarketAsset,
    pub claim_amount: u64,
    pub buffer_share_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: UnstakeArgs)]
pub struct Unstake<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub claim_token_mint: Box<InterfaceAccount<'info, Mint>>,

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

impl<'info> Unstake<'info> {
    pub fn validate(&self, args: &UnstakeArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(
            args.claim_amount > 0 && args.buffer_share_amount > 0,
            ErrorCode::AmountZero
        );
        validate_stake_accounts(
            &self.market,
            args.market_asset,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_token_mint,
            &self.stake_vault,
            &self.owner_claim_account,
        )?;
        require_fee_free_claim_token_mint(&self.claim_token_mint)?;
        self.stake_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )?;
        require_gte!(
            self.stake_position.staked_claim_token_amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            self.stake_position.staked_buffer_share_amount,
            args.buffer_share_amount,
            ErrorCode::InsufficientBufferShares
        );
        Ok(())
    }

    pub fn handle_unstake(ctx: Context<Self>, args: UnstakeArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let stake_receipt = {
            let market_side = ctx.accounts.market.side_mut(args.market_asset)?;
            UnstakeTransition::new(args.claim_amount, args.buffer_share_amount)
                .apply(market_side, &mut ctx.accounts.stake_position)?
        };

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.claim_token_mint.to_account_info(),
            claim_token_program,
            args.claim_amount,
            ctx.accounts.claim_token_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketStakeUpdated {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            staked_claim_token_amount: stake_receipt.staked_claim_token_amount,
            staked_buffer_share_amount: stake_receipt.staked_buffer_share_amount,
            active_stake_units: stake_receipt.active_stake_units,
            accrued_fee_amount: stake_receipt.accrued_fee_amount,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}
