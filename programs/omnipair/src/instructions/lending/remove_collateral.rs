use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::{AdjustCollateralEvent, EventMetadata, UserPositionUpdatedEvent},
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{CommonAdjustPosition, AdjustPositionArgs},
};

use crate::state::{Pair, UserPosition};

fn calculate_max_withdrawable(pair: &Pair, user_position: &UserPosition, is_collateral_token0: bool) -> Result<u64> {
    let user_collateral = match is_collateral_token0 {
        true => user_position.collateral0,
        false => user_position.collateral1,
    };

    // Calculate current debt
    let debt = match is_collateral_token0 {
        true => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        false => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
    };

    // If no debt, can withdraw all collateral
    if debt == 0 {
        return Ok(user_collateral);
    }

    // Calculate required collateral for current debt
    let debt_token = if is_collateral_token0 { pair.token1 } else { pair.token0 };
    let collateral_token = pair.get_collateral_token(&debt_token);
    let collateral_amount = match collateral_token == pair.token0 {
        true => user_position.collateral0,
        false => user_position.collateral1,
    };
    let pessimistic_cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(pair, &collateral_token, collateral_amount)?.1;
    
    // Calculate minimum required collateral value in debt token
    let min_collateral_value = (debt as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(pessimistic_cf_bps as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?;
 
    // Convert collateral value back to collateral token amount
    let collateral_price = if is_collateral_token0 {
        pair.ema_price0_nad()
    } else {
        pair.ema_price1_nad()
    };
 
    // minimum collateral to cover outstanding debt
    let min_collateral = (min_collateral_value as u128)
        .checked_mul(NAD as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(collateral_price as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?;
 
    // Calculate maximum withdrawable amount
    let max_withdrawable = user_collateral
        .checked_sub(min_collateral as u64)
        .ok_or(ErrorCode::DebtMathOverflow)?;
    
    Ok(max_withdrawable)
}

impl<'info> CommonAdjustPosition<'info> {
    pub fn validate_remove(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);

        let collateral_token = self.user_token_account.mint;
        let is_collateral_token0 = collateral_token == self.pair.token0;
        let is_withdraw_all = args.amount == u64::MAX;
        let user_collateral = match is_collateral_token0 {
            true => self.user_position.collateral0,
            false => self.user_position.collateral1,
        };

        // Calculate current debt
        let debt = match is_collateral_token0 {
            true => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
            false => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
        };

        // If no debt, can withdraw all collateral
        if debt == 0 {
            return Ok(());
        }

        // Calculate maximum withdrawable amount
        let max_withdrawable = calculate_max_withdrawable(&self.pair, &self.user_position, is_collateral_token0)?;
        let withdraw_amount = if is_withdraw_all { max_withdrawable } else { *amount };
        
        // Check if user has enough collateral
        match is_collateral_token0 {
            true => {
                require_gte!(
                    user_collateral,
                    withdraw_amount,
                    ErrorCode::InsufficientCollateral
                );
            },
            false => {
                require_gte!(
                    user_collateral,
                    withdraw_amount,
                    ErrorCode::InsufficientCollateral
                );
            }
        }

        // Ensure withdrawal amount doesn't exceed maximum withdrawable
        require_gte!(
            max_withdrawable,
            withdraw_amount,
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

        let is_withdraw_all = args.amount == u64::MAX;
        let is_token0 = user_token_account.mint == pair.token0;
        // Calculate current debt
        let debt = match is_token0 {
            true => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
            false => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
        };

        let withdraw_amount = if !is_withdraw_all { args.amount } else { 
            if debt == 0 {
                match is_token0 {
                    true => user_position.collateral0,
                    false => user_position.collateral1,
                }
            } else {
                // Calculate maximum withdrawable amount
                let max_withdrawable = calculate_max_withdrawable(&pair, &user_position, is_token0)?;
                max_withdrawable
            }
         }; 

        transfer_from_pool_vault_to_user(
            pair.to_account_info(),
            token_vault.to_account_info(),
            user_token_account.to_account_info(),
            vault_token_mint.to_account_info(),
            match vault_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            withdraw_amount,
            vault_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Transfer tokens from vault to user
        match is_token0 {
            true => {
                pair.total_collateral0 = pair.total_collateral0.checked_sub(withdraw_amount).unwrap();
                user_position.collateral0 = user_position.collateral0.checked_sub(withdraw_amount).unwrap();
            },
            false => {
                pair.total_collateral1 = pair.total_collateral1.checked_sub(withdraw_amount).unwrap();
                user_position.collateral1 = user_position.collateral1.checked_sub(withdraw_amount).unwrap();
            }
        }

        // Emit collateral adjustment event
        let (amount0, amount1) = match is_token0 {
            true => (-(withdraw_amount as i64), 0),
            false => (0, -(withdraw_amount as i64)),
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
