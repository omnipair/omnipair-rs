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

        let collateral_token = self.user_token_account.mint;
        let is_collateral_token0 = collateral_token == self.pair.token0;
        
        // Check if user has enough collateral
        match is_collateral_token0 {
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

        // Calculate current debt
        let debt = match is_collateral_token0 {
            true => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
            false => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
        };

        // If no debt, can withdraw all collateral
        if debt == 0 {
            return Ok(());
        }

        // Calculate required collateral for current debt
        let debt_token = if is_collateral_token0 { self.pair.token1 } else { self.pair.token0 };
        let effective_cf_bps = self.user_position.get_effective_collateral_factor_bps(&self.pair, &debt_token);
        
        // Calculate minimum required collateral value in debt token
        let min_collateral_value = (debt as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(effective_cf_bps as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        // Convert collateral value back to collateral token amount
        let collateral_price = if is_collateral_token0 {
            self.pair.ema_price0_nad()
        } else {
            self.pair.ema_price1_nad()
        };

        let min_collateral = (min_collateral_value as u128)
            .checked_mul(NAD as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(collateral_price as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        // Calculate current collateral
        let current_collateral = if is_collateral_token0 {
            self.user_position.collateral0
        } else {
            self.user_position.collateral1
        };

        // Calculate maximum withdrawable amount
        let max_withdrawable = current_collateral
            .checked_sub(min_collateral as u64)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        // Ensure withdrawal amount doesn't exceed maximum withdrawable
        require_gte!(
            max_withdrawable,
            *amount,
            ErrorCode::BorrowingPowerExceeded
        );
        
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
