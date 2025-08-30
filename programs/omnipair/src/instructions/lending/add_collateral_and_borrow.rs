use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use crate::{
    errors::ErrorCode,
    events::{AdjustCollateralEvent, AdjustDebtEvent, UserPositionCreatedEvent, UserPositionUpdatedEvent},
    utils::{token::{transfer_from_user_to_pool_vault, transfer_from_pool_vault_to_user}, account::get_size_with_discriminator},
    state::{user_position::UserPosition, pair::Pair, rate_model::RateModel},
    constants::*,
    generate_gamm_pair_seeds,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddCollateralAndBorrowArgs {
    pub collateral_amount: u64,
    pub borrow_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct AddCollateralAndBorrow<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

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

    // Collateral accounts
    #[account(
        mut,
        constraint = collateral_vault.mint == pair.token0 || collateral_vault.mint == pair.token1,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_collateral_token_account.mint == pair.token0 || user_collateral_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_collateral_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = collateral_vault.mint)]
    pub collateral_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Borrow accounts
    #[account(
        mut,
        constraint = borrow_vault.mint == pair.token0 || borrow_vault.mint == pair.token1,
    )]
    pub borrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_borrow_token_account.mint == pair.token0 || user_borrow_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_borrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = borrow_vault.mint)]
    pub borrow_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddCollateralAndBorrow<'info> {
    pub fn validate_add_collateral_and_borrow(&self, args: &AddCollateralAndBorrowArgs) -> Result<()> {
        let AddCollateralAndBorrowArgs { collateral_amount, borrow_amount } = args;
        
        require!(*collateral_amount > 0, ErrorCode::AmountZero);
        require!(*borrow_amount > 0, ErrorCode::AmountZero);
        
        // Ensure user has sufficient collateral tokens
        require_gte!(
            self.user_collateral_token_account.amount,
            *collateral_amount,
            ErrorCode::InsufficientBalanceForCollateral
        );

        // Ensure collateral and borrow tokens are different
        require!(
            self.collateral_vault.mint != self.borrow_vault.mint,
            ErrorCode::InvalidTokenOrder
        );

        // Ensure user didn't add collateral yet
        match self.user_collateral_token_account.mint == self.pair.token0 {
            true => {
                require!(
                    self.user_position.collateral0 == 0,
                    ErrorCode::UserAlreadyAddedCollateral
                );
            },
            false => {
                require!(
                    self.user_position.collateral1 == 0,
                    ErrorCode::UserAlreadyAddedCollateral
                );
            }
        }

        Ok(())
    }
    
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }

    pub fn update_and_validate_borrow(&mut self, args: &AddCollateralAndBorrowArgs) -> Result<()> {
        self.update()?;
        self.validate_add_collateral_and_borrow(args)?;
        Ok(())
    }

    pub fn handle_add_collateral_and_borrow(ctx: Context<Self>, args: AddCollateralAndBorrowArgs) -> Result<()> {
        let AddCollateralAndBorrow { 
            pair, 
            user, 
            collateral_vault,
            collateral_token_mint,
            borrow_vault,
            borrow_token_mint,
            token_program,
            user_collateral_token_account,
            user_borrow_token_account,
            user_position,
            token_2022_program,
            ..
        } = ctx.accounts;

        // Initialize user position if it doesn't exist
        if !user_position.is_initialized() {
            user_position.initialize(
                user.key(),
                pair.key(),
                ctx.bumps.user_position,
            )?;

            emit_cpi!(UserPositionCreatedEvent {
                user: user.key(),
                pair: pair.key(),
                position: user_position.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        // Add collateral
        let is_collateral_token0 = user_collateral_token_account.mint == pair.token0;
        
        // Transfer collateral tokens from user to vault
        transfer_from_user_to_pool_vault(
            user.to_account_info(),
            user_collateral_token_account.to_account_info(),
            collateral_vault.to_account_info(),
            collateral_token_mint.to_account_info(),
            match collateral_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            args.collateral_amount,
            collateral_token_mint.decimals,
        )?;
        
        // Update collateral amounts
        match is_collateral_token0 {
            true => {
                pair.total_collateral0 = pair.total_collateral0.checked_add(args.collateral_amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_add(args.collateral_amount).unwrap();
            },
            false => {
                pair.total_collateral1 = pair.total_collateral1.checked_add(args.collateral_amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_add(args.collateral_amount).unwrap();
            }
        }


        // Borrow
        let user_debt = match user_borrow_token_account.mint == pair.token0 {
            true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
            false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        };

        let (borrow_limit, applied_min_cf_bps) = user_position.get_user_borrow_limit_and_cf_bps(&pair, &borrow_token_mint.key());
        let is_max_borrow = args.borrow_amount == u64::MAX;
        let remaining_borrow_limit = borrow_limit.checked_sub(user_debt).ok_or(ErrorCode::DebtMathOverflow)?;
        let borrow_amount = if is_max_borrow { remaining_borrow_limit } else { args.borrow_amount };
        
        let new_debt = user_debt
            .checked_add(borrow_amount)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        require_gte!(
            borrow_limit,
            new_debt,
            ErrorCode::BorrowingPowerExceeded
        );

        let is_borrow_token0 = user_borrow_token_account.mint == pair.token0;
        
        // Transfer tokens from vault to user
        transfer_from_pool_vault_to_user(
            pair.to_account_info(),
            borrow_vault.to_account_info(),
            user_borrow_token_account.to_account_info(),
            borrow_token_mint.to_account_info(),
            match borrow_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            borrow_amount,
            borrow_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        user_position.increase_debt(pair, &borrow_token_mint.key(), borrow_amount)?;
        // Update user position fixed CF
        user_position.set_applied_min_cf_for_debt_token(&borrow_token_mint.key(), &pair, applied_min_cf_bps);

        // Emit collateral adjustment event
        let (collateral_amount0, collateral_amount1) = if is_collateral_token0 {
            (args.collateral_amount as i64, 0)
        } else {
            (0, args.collateral_amount as i64)
        };
        
        emit_cpi!(AdjustCollateralEvent {
            user: user.key(),
            amount0: collateral_amount0,
            amount1: collateral_amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Emit debt adjustment event
        let (borrow_amount0, borrow_amount1) = if is_borrow_token0 {
            (borrow_amount as i64, 0)
        } else {
            (0, borrow_amount as i64)
        };
        
        emit_cpi!(AdjustDebtEvent {
            user: user.key(),
            amount0: borrow_amount0,
            amount1: borrow_amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Emit position updated event
        emit_cpi!(UserPositionUpdatedEvent {
            user: user.key(),
            pair: pair.key(),
            position: user_position.key(),
            collateral0: user_position.collateral0,
            collateral1: user_position.collateral1,
            debt0_shares: user_position.debt0_shares,
            debt1_shares: user_position.debt1_shares,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
