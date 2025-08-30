use anchor_lang::prelude::*;
use crate::{
    errors::ErrorCode,
    events::{AdjustDebtEvent, UserPositionUpdatedEvent},
    utils::token::transfer_from_user_to_pool_vault,
    instructions::lending::common::{CommonAdjustPosition, AdjustPositionArgs},
};

impl<'info> CommonAdjustPosition<'info> {
    pub fn validate_repay(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);

        let is_repay_all = *amount == u64::MAX;
        let is_token0 = self.user_token_account.mint == self.pair.token0;
        let user_total_debt = match is_token0 {
            true => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
            false => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
        };
        let debt_to_repay = if is_repay_all { user_total_debt } else { *amount };
        
        // Check user token balance >= debt to repay
        require_gte!(
            self.user_token_account.amount,
            debt_to_repay,
            ErrorCode::InsufficientAmount
        );

        // Check user debt >= debt to repay
        require_gte!(
            user_total_debt,
            debt_to_repay,
            ErrorCode::InsufficientDebt
        );
        
        // debt cannot be zero
        require_gt!(
            user_total_debt,
            0,
            ErrorCode::ZeroDebtAmount
        );
        
        Ok(())
    }

    pub fn update_and_validate_repay(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.update()?;
        self.validate_repay(args)?;
        Ok(())
    }

    pub fn handle_repay(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
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

        let is_repay_all = args.amount == u64::MAX;
        let is_token0 = user_token_account.mint == pair.token0;
        let debt_to_repay = if is_repay_all { 
            match is_token0 {
                true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
                false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
            }
        } else {
            args.amount
        };

        // Transfer tokens from user to vault
        transfer_from_user_to_pool_vault(
            user.to_account_info(),
            user_token_account.to_account_info(),
            token_vault.to_account_info(),
            vault_token_mint.to_account_info(),
            match vault_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            debt_to_repay,
            vault_token_mint.decimals,
        )?;

        
        // Update debt
        match is_token0 {
            true => {
                let shares = if is_repay_all {
                    user_position.debt0_shares
                } else {
                    debt_to_repay
                    .checked_mul(pair.total_debt0_shares)
                    .unwrap()
                    .checked_div(pair.total_debt0)
                    .unwrap()
                };
                    
                pair.total_debt0_shares = pair.total_debt0_shares.checked_sub(shares).unwrap();
                pair.total_debt0 = pair.total_debt0.checked_sub(debt_to_repay).unwrap();
                user_position.debt0_shares = user_position.debt0_shares.checked_sub(shares).unwrap();
            },
            false => {
                let shares = if is_repay_all {
                    user_position.debt1_shares
                } else {
                    debt_to_repay
                    .checked_mul(pair.total_debt1_shares)
                    .unwrap()
                    .checked_div(pair.total_debt1)
                    .unwrap()
                };
                pair.total_debt1_shares = pair.total_debt1_shares.checked_sub(shares).unwrap();
                pair.total_debt1 = pair.total_debt1.checked_sub(debt_to_repay).unwrap();
                user_position.debt1_shares = user_position.debt1_shares.checked_sub(shares).unwrap();
            }
        }
        

        // Emit event
        let (amount0, amount1) = if user_token_account.mint == pair.token0 {
            (-(debt_to_repay as i64), 0)
        } else {
            (0, -(debt_to_repay as i64))
        };
        
        emit_cpi!(AdjustDebtEvent {
            user: user.key(),
            amount0,
            amount1,
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
