use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::AdjustCollateralEvent,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{CommonAdjustPosition, AdjustPositionArgs},
};

impl<'info> CommonAdjustPosition<'info> {
    pub fn validate_remove(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);
        
        // Check if user has enough collateral
        match self.user_token_account.mint == self.pair.token0 {
            true => {
                require_gte!(
                    self.user_position.collateral0,
                    *amount,
                    ErrorCode::InsufficientCollateral
                );
            },
            false => {
                require_gte!(
                    self.user_position.collateral1,
                    *amount,
                    ErrorCode::InsufficientCollateral
                );
            }
        }
        
        Ok(())
    }

    pub fn update_and_validate_remove(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.update()?;
        self.validate_remove(args)?;
        Ok(())
    }

    pub fn handle_remove_collateral(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
        let CommonAdjustPosition {
            pair,
            token_vault,
            user_token_account,
            vault_token_mint,
            token_program,
            token_2022_program,
            user,
            user_position,
            ..
        } = ctx.accounts;

        // TODO: check if collateral is undercollateralized

        // Transfer tokens from vault to user
        match user_token_account.mint == pair.token0 {
            true => {
                transfer_from_pool_vault_to_user(
                    pair.to_account_info(),
                    token_vault.to_account_info(),
                    user_token_account.to_account_info(),
                    vault_token_mint.to_account_info(),
                    match vault_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    vault_token_mint.decimals,
                    &[&generate_gamm_pair_seeds!(pair)[..]],
                )?;
                
                // Update collateral
                pair.total_collateral0 = pair.total_collateral0.checked_sub(args.amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_sub(args.amount).unwrap();
            },
            false => {
                transfer_from_pool_vault_to_user(
                    pair.to_account_info(),
                    token_vault.to_account_info(),
                    user_token_account.to_account_info(),
                    vault_token_mint.to_account_info(),
                    match vault_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    vault_token_mint.decimals,
                    &[&generate_gamm_pair_seeds!(pair)[..]],
                )?;
                
                // Update collateral
                pair.total_collateral1 = pair.total_collateral1.checked_sub(args.amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_sub(args.amount).unwrap();
            }
        }

        // Emit event
        let (amount0, amount1) = if user_token_account.mint == pair.token0 {
            (-(args.amount as i64), 0)
        } else {
            (0, -(args.amount as i64))
        };
        
        emit!(AdjustCollateralEvent {
            user: user.key(),
            amount0,
            amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
