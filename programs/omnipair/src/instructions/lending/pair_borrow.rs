use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::AdjustDebtEvent,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{AdjustDebt, AdjustDebtArgs},
};

impl<'info> AdjustDebt<'info> {
    pub fn validate_borrow(&self, args: &AdjustDebtArgs) -> Result<()> {
        let AdjustDebtArgs { amount0, amount1 } = args;
        
        require!(*amount0 > 0 || *amount1 > 0, ErrorCode::AmountZero);
        
        if *amount0 > 0 {
            require_gte!(
                self.token0_vault.amount,
                *amount0,
                ErrorCode::InsufficientAmount0
            );
        }
        
        if *amount1 > 0 {
            require_gte!(
                self.token1_vault.amount,
                *amount1,
                ErrorCode::InsufficientAmount1
            );
        }
        
        Ok(())
    }

    pub fn validate_borrow_and_update(&mut self, args: &AdjustDebtArgs) -> Result<()> {
        self.validate_borrow(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_borrow(ctx: Context<Self>, args: AdjustDebtArgs) -> Result<()> {
        let AdjustDebt {
            pair,
            token0_vault,
            token1_vault,
            user_token0_account,
            user_token1_account,
            token0_vault_mint,
            token1_vault_mint,
            token_program,
            token_2022_program,
            user,
            ..
        } = ctx.accounts;

        // Update pair state
        pair.update(&ctx.accounts.rate_model)?;

        // Check borrowing power
        if args.amount0 > 0 {
            let borrowing_power1 = pair.total_collateral1
                .checked_mul(pair.price1_mantissa())
                .unwrap()
                .checked_mul(CF_BPS)
                .unwrap()
                .checked_div(SCALE)
                .unwrap()
                .checked_div(10000)
                .unwrap();
            
            let new_debt0 = pair.total_debt0.checked_add(args.amount0).unwrap();
            require!(
                new_debt0 <= borrowing_power1,
                ErrorCode::BorrowingPowerExceeded
            );
        }

        if args.amount1 > 0 {
            let borrowing_power0 = pair.total_collateral0
                .checked_mul(pair.price0_mantissa())
                .unwrap()
                .checked_mul(CF_BPS)
                .unwrap()
                .checked_div(SCALE)
                .unwrap()
                .checked_div(10000)
                .unwrap();
            
            let new_debt1 = pair.total_debt1.checked_add(args.amount1).unwrap();
            require!(
                new_debt1 <= borrowing_power0,
                ErrorCode::BorrowingPowerExceeded
            );
        }

        // Transfer tokens from vault to user
        if args.amount0 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token0_vault.to_account_info(),
                user_token0_account.to_account_info(),
                token0_vault_mint.to_account_info(),
                match token0_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount0,
                token0_vault_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
            
            // Update debt
            if pair.total_debt0_shares == 0 {
                pair.total_debt0_shares = args.amount0;
            } else {
                let shares = args.amount0
                    .checked_mul(pair.total_debt0_shares)
                    .unwrap()
                    .checked_div(pair.total_debt0)
                    .unwrap();
                pair.total_debt0_shares = pair.total_debt0_shares.checked_add(shares).unwrap();
            }
            pair.total_debt0 = pair.total_debt0.checked_add(args.amount0).unwrap();
        }

        if args.amount1 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token1_vault.to_account_info(),
                user_token1_account.to_account_info(),
                token1_vault_mint.to_account_info(),
                match token1_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount1,
                token1_vault_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
            
            // Update debt
            if pair.total_debt1_shares == 0 {
                pair.total_debt1_shares = args.amount1;
            } else {
                let shares = args.amount1
                    .checked_mul(pair.total_debt1_shares)
                    .unwrap()
                    .checked_div(pair.total_debt1)
                    .unwrap();
                pair.total_debt1_shares = pair.total_debt1_shares.checked_add(shares).unwrap();
            }
            pair.total_debt1 = pair.total_debt1.checked_add(args.amount1).unwrap();
        }

        // Emit event
        emit!(AdjustDebtEvent {
            user: user.key(),
            amount0: args.amount0 as i64,
            amount1: args.amount1 as i64,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
