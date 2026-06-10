use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{
        MarketEventMetadataV2, MarketFeeLiabilityClaimedV2, MarketFeesClaimedV2,
        MarketStakeUpdatedV2,
    },
    generate_market_v2_seeds,
    state::{MarketFeeClaimKindV2, MarketV2, StakePositionV2},
    utils::{
        token::{transfer_from_user_to_vault, transfer_from_vault_to_user},
    },
    v2::utils::market_math::active_stake_units,
};

use super::common::{
    require_fee_free_claim_mint, require_supported_asset_mint, token_program_for_mint,
    validate_fee_accounts, validate_stake_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct StakeV2Args {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub buffer_shares: u64,
    pub min_active_stake_units: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UnstakeV2Args {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub buffer_shares: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimFeesV2Args {
    pub market_side_index: u8,
    pub min_fee_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimMarketFeesV2Args {
    pub market_side_index: u8,
    pub claim_kind: MarketFeeClaimKindV2,
    pub min_fee_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: StakeV2Args)]
pub struct StakeV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

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
            STAKE_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = stake_position.bump
    )]
    pub stake_position: Box<Account<'info, StakePositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> StakeV2<'info> {
    pub fn validate(&self, args: &StakeV2Args) -> Result<()> {
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
            ErrorCode::InsufficientBufferSharesV2
        );
        let next_active_units = active_stake_units(
            self.stake_position
                .staked_claim_amount
                .checked_add(args.claim_amount)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
            self.stake_position
                .staked_buffer_shares
                .checked_add(args.buffer_shares)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
            market_side.buffer_book.buffer_ratio_bps,
        )?;
        require_gte!(
            next_active_units,
            args.min_active_stake_units,
            ErrorCode::SlippageExceeded
        );
        Ok(())
    }

    pub fn handle_stake(ctx: Context<Self>, args: StakeV2Args) -> Result<()> {
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
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
            market_side.buffer_book.staked_buffer_shares = market_side
                .buffer_book
                .staked_buffer_shares
                .checked_add(args.buffer_shares)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
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

        emit_cpi!(MarketStakeUpdatedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            staked_claim_amount,
            staked_buffer_shares,
            active_stake_units: active_units,
            accrued_fee_amount,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: UnstakeV2Args)]
pub struct UnstakeV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

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
            STAKE_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = stake_position.bump
    )]
    pub stake_position: Box<Account<'info, StakePositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> UnstakeV2<'info> {
    pub fn validate(&self, args: &UnstakeV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(
            args.claim_amount > 0 && args.buffer_shares > 0,
            ErrorCode::AmountZero
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
        require_gte!(
            self.stake_position.staked_claim_amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            self.stake_position.staked_buffer_shares,
            args.buffer_shares,
            ErrorCode::InsufficientBufferSharesV2
        );
        Ok(())
    }

    pub fn handle_unstake(ctx: Context<Self>, args: UnstakeV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let (active_units, accrued_fee_amount, staked_claim_amount, staked_buffer_shares) = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.carry_forward_unallocated_fee()?;
            ctx.accounts.stake_position.accrue_fees(
                market_side.fee_ledger.fee_growth_index_nad,
                market_side.buffer_book.buffer_ratio_bps,
            )?;
            ctx.accounts
                .stake_position
                .unstake(args.claim_amount, args.buffer_shares)?;
            market_side.claim_ledger.staked_claim_supply = market_side
                .claim_ledger
                .staked_claim_supply
                .checked_sub(args.claim_amount)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
            market_side.buffer_book.staked_buffer_shares = market_side
                .buffer_book
                .staked_buffer_shares
                .checked_sub(args.buffer_shares)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
            (
                ctx.accounts
                    .stake_position
                    .active_stake_units(market_side.buffer_book.buffer_ratio_bps)?,
                ctx.accounts.stake_position.accrued_fee_amount,
                ctx.accounts.stake_position.staked_claim_amount,
                ctx.accounts.stake_position.staked_buffer_shares,
            )
        };

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.claim_mint.to_account_info(),
            claim_token_program,
            args.claim_amount,
            ctx.accounts.claim_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketStakeUpdatedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            staked_claim_amount,
            staked_buffer_shares,
            active_stake_units: active_units,
            accrued_fee_amount,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimFeesV2Args)]
pub struct ClaimFeesV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_fee_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            STAKE_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = stake_position.bump
    )]
    pub stake_position: Box<Account<'info, StakePositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimFeesV2<'info> {
    pub fn validate(&self, args: &ClaimFeesV2Args) -> Result<()> {
        self.market.assert_started()?;
        validate_fee_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.fee_vault,
            &self.owner_fee_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        self.stake_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimFeesV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let fee_amount = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.carry_forward_unallocated_fee()?;
            ctx.accounts.stake_position.accrue_fees(
                market_side.fee_ledger.fee_growth_index_nad,
                market_side.buffer_book.buffer_ratio_bps,
            )?;
            let fee_amount = ctx.accounts.stake_position.accrued_fee_amount;
            require!(fee_amount > 0, ErrorCode::AmountZero);
            require_gte!(fee_amount, args.min_fee_amount, ErrorCode::SlippageExceeded);
            require_gte!(
                market_side.fee_ledger.fee_liability,
                fee_amount,
                ErrorCode::UnbackedFeeLiabilityV2
            );
            require_gte!(
                ctx.accounts.fee_vault.amount,
                fee_amount,
                ErrorCode::UnbackedFeeLiabilityV2
            );
            fee_amount
        };

        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.fee_vault.to_account_info(),
            ctx.accounts.owner_fee_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            fee_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.fee_vault.reload()?;

        let remaining_fee_liability = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.fee_ledger.fee_liability = market_side
                .fee_ledger
                .fee_liability
                .checked_sub(fee_amount)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
            market_side.fee_ledger.fee_vault_balance = ctx.accounts.fee_vault.amount;
            ctx.accounts.stake_position.accrued_fee_amount = 0;
            market_side.fee_ledger.fee_liability
        };

        emit_cpi!(MarketFeesClaimedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            fee_amount,
            remaining_fee_liability,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ClaimMarketFeesV2Args)]
pub struct ClaimMarketFeesV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub fee_authority: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub recipient_fee_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimMarketFeesV2<'info> {
    pub fn validate(&self, args: &ClaimMarketFeesV2Args) -> Result<()> {
        self.market.assert_started()?;
        match args.claim_kind {
            MarketFeeClaimKindV2::Operator => require_keys_eq!(
                self.fee_authority.key(),
                self.market.operator,
                ErrorCode::InvalidMarketFeeAuthorityV2
            ),
            MarketFeeClaimKindV2::Protocol => require_keys_eq!(
                self.fee_authority.key(),
                self.market.manager,
                ErrorCode::InvalidMarketFeeAuthorityV2
            ),
        }
        validate_fee_accounts(
            &self.market,
            args.market_side_index,
            self.fee_authority.key(),
            &self.asset_mint,
            &self.fee_vault,
            &self.recipient_fee_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimMarketFeesV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let fee_authority_key = ctx.accounts.fee_authority.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let fee_amount = {
            let market_side = ctx.accounts.market.side(args.market_side_index)?;
            let fee_amount = market_side.fee_ledger.market_fee_liability(args.claim_kind);
            require!(fee_amount > 0, ErrorCode::AmountZero);
            require_gte!(fee_amount, args.min_fee_amount, ErrorCode::SlippageExceeded);
            require_gte!(
                ctx.accounts.fee_vault.amount,
                fee_amount,
                ErrorCode::UnbackedFeeLiabilityV2
            );
            fee_amount
        };

        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.fee_vault.to_account_info(),
            ctx.accounts.recipient_fee_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            fee_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.fee_vault.reload()?;

        let remaining_fee_liability = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            let claimed_amount = market_side
                .fee_ledger
                .claim_market_fee_liability(args.claim_kind)?;
            require_eq!(
                claimed_amount,
                fee_amount,
                ErrorCode::UnbackedFeeLiabilityV2
            );
            market_side.fee_ledger.fee_vault_balance = ctx.accounts.fee_vault.amount;
            market_side.fee_ledger.assert_backed()?;
            market_side.fee_ledger.market_fee_liability(args.claim_kind)
        };

        emit_cpi!(MarketFeeLiabilityClaimedV2 {
            market: market_key,
            authority: fee_authority_key,
            asset_mint: asset_mint_key,
            claim_kind: args.claim_kind.event_code(),
            fee_amount,
            remaining_fee_liability,
            metadata: MarketEventMetadataV2::new(fee_authority_key, market_key),
        });

        Ok(())
    }
}
