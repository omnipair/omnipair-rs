use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
};
use crate::{
    errors::ErrorCode,
    events::{AdjustCollateralEvent, UserPositionCreatedEvent, UserPositionUpdatedEvent, EventMetadata},
    utils::{token::transfer_from_user_to_vault, account::get_size_with_discriminator},
    instructions::lending::common::AdjustCollateralArgs,
    state::{user_position::UserPosition, pair::Pair, rate_model::RateModel, futarchy_authority::FutarchyAuthority},
    constants::*,
};

#[event_cpi]
#[derive(Accounts)]
pub struct AddCollateral<'info> {
    #[account(
        mut,
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
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init_if_needed,
        payer = user,
        space = get_size_with_discriminator::<UserPosition>(),
        constraint = user_position.owner == Pubkey::default() || user_position.owner == user.key(),
        constraint = user_position.pair == Pubkey::default() || user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(
        mut,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_collateral_vault_bump(&collateral_token_mint.key())
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_collateral_token_account.mint == pair.token0 || user_collateral_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_collateral_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddCollateral<'info> {
    pub fn validate_add(&self, args: &AdjustCollateralArgs) -> Result<()> {
        let AdjustCollateralArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);
        
        require_gte!(
            self.user_collateral_token_account.amount,
            *amount,
            ErrorCode::InsufficientBalanceForCollateral
        );
        
        Ok(())
    }
    
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }

    pub fn update_and_validate_add(&mut self, args: &AdjustCollateralArgs) -> Result<()> {
        self.update()?;
        self.validate_add(args)?;
        Ok(())
    }

    pub fn handle_add_collateral(ctx: Context<Self>, args: AdjustCollateralArgs) -> Result<()> {
        let AddCollateral { 
            pair, 
            user, 
            collateral_vault,
            collateral_token_mint,
            token_program,
            user_collateral_token_account,
            user_position,
            token_2022_program,
            ..
        } = ctx.accounts;

        if !user_position.is_initialized() {
            user_position.initialize(
                user.key(),
                pair.key(),
                ctx.bumps.user_position,
            )?;

            emit_cpi!(UserPositionCreatedEvent {
                metadata: EventMetadata::new(user.key(), pair.key()),
                position: user_position.key(),
            });
        }

        // Transfer tokens from user to collateral vault
        let is_collateral_token0 = user_collateral_token_account.mint == pair.token0;

        transfer_from_user_to_vault(
            user.to_account_info(),
            user_collateral_token_account.to_account_info(),
            collateral_vault.to_account_info(),
            collateral_token_mint.to_account_info(),
            match collateral_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            args.amount,
            collateral_token_mint.decimals,
        )?;

        match is_collateral_token0 {
            true => {
                pair.total_collateral0 = pair.total_collateral0.checked_add(args.amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_add(args.amount).unwrap();
            },
            false => {
                pair.total_collateral1 = pair.total_collateral1.checked_add(args.amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_add(args.amount).unwrap();
            }
        }
        
        let collateral_token = if is_collateral_token0 { pair.token0 } else { pair.token1 };
        let debt_token = if is_collateral_token0 { pair.token1 } else { pair.token0 };
        let collateral_amount = if is_collateral_token0 {
            user_position.collateral0
        } else {
            user_position.collateral1
        };
        let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount)?;
        user_position.set_applied_min_cf_for_debt_token(&debt_token, &pair, liquidation_cf_bps);

        // Emit collateral adjustment event
        let (amount0, amount1) = if user_collateral_token_account.mint == pair.token0 {
            (args.amount as i64, 0)
        } else {
            (0, args.amount as i64)
        };
        
        emit_cpi!(AdjustCollateralEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        });

        // Emit position updated event
        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            position: user_position.key(),
            collateral0: user_position.collateral0,
            collateral1: user_position.collateral1,
            debt0_shares: user_position.debt0_shares,
            debt1_shares: user_position.debt1_shares,
            collateral0_applied_min_cf_bps: user_position.collateral0_applied_min_cf_bps,
            collateral1_applied_min_cf_bps: user_position.collateral1_applied_min_cf_bps,
        });

        Ok(())
    }
}
