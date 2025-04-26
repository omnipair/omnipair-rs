use anchor_lang::prelude::*;
use crate::{

    errors::ErrorCode,
    events::AdjustDebtEvent,
    utils::token::transfer_from_user_to_pool_vault,
    instructions::lending::common::{AdjustDebt, AdjustDebtArgs},
};

impl<'info> AdjustDebt<'info> {
    pub fn validate_repay(&self, args: &AdjustDebtArgs) -> Result<()> {
        let AdjustDebtArgs { amount0, amount1 } = args;
        
        require!(*amount0 > 0 || *amount1 > 0, ErrorCode::AmountZero);
        
        if *amount0 > 0 {
            require_gte!(
                self.user_token0_account.amount,
                *amount0,
                ErrorCode::InsufficientAmount0
            );
        }
        
        if *amount1 > 0 {
            require_gte!(
                self.user_token1_account.amount,
                *amount1,
                ErrorCode::InsufficientAmount1
            );
        }
        
        Ok(())
    }

    pub fn validate_repay_and_update(&mut self, args: &AdjustDebtArgs) -> Result<()> {
        self.validate_repay(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_repay(ctx: Context<Self>, args: AdjustDebtArgs) -> Result<()> {
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

        // Transfer tokens from user to vault
        if args.amount0 > 0 {
            transfer_from_user_to_pool_vault(
                user.to_account_info(),
                user_token0_account.to_account_info(),
                token0_vault.to_account_info(),
                token0_vault_mint.to_account_info(),
                match token0_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount0,
                token0_vault_mint.decimals,
            )?;
            
            // Update debt
            let shares = args.amount0
                .checked_mul(pair.total_debt0_shares)
                .unwrap()
                .checked_div(pair.total_debt0)
                .unwrap();
            pair.total_debt0_shares = pair.total_debt0_shares.checked_sub(shares).unwrap();
            pair.total_debt0 = pair.total_debt0.checked_sub(args.amount0).unwrap();
        }

        if args.amount1 > 0 {
            transfer_from_user_to_pool_vault(
                user.to_account_info(),
                user_token1_account.to_account_info(),
                token1_vault.to_account_info(),
                token1_vault_mint.to_account_info(),
                match token1_vault_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                args.amount1,
                token1_vault_mint.decimals,
            )?;
            
            // Update debt
            let shares = args.amount1
                .checked_mul(pair.total_debt1_shares)
                .unwrap()
                .checked_div(pair.total_debt1)
                .unwrap();
            pair.total_debt1_shares = pair.total_debt1_shares.checked_sub(shares).unwrap();
            pair.total_debt1 = pair.total_debt1.checked_sub(args.amount1).unwrap();
        }

        // Emit event
        emit!(AdjustDebtEvent {
            user: user.key(),
            amount0: -(args.amount0 as i64),
            amount1: -(args.amount1 as i64),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
