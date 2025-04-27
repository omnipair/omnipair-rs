use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::AdjustDebtEvent,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{CommonAdjustPosition, AdjustPositionArgs},
};

impl<'info> CommonAdjustPosition<'info> {
    pub fn validate_borrow(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount: amount_out } = args;
        
        require!(*amount_out > 0, ErrorCode::AmountZero);
        
        // Check if vault has enough tokens
        require_gte!(
            self.token_vault.amount,
            *amount_out,
            ErrorCode::InsufficientAmount
        );
        
        // Check borrowing power
        match self.user_token_account.mint == self.pair.token0 {
            true => {
                let borrowing_power1 = self.pair.total_collateral1
                    .checked_mul(self.pair.price1_mantissa())
                    .unwrap()
                    .checked_div(SCALE)
                    .unwrap()
                    .checked_div(10000)
                    .unwrap();
                
                let new_debt0 = self.pair.total_debt0.checked_add(*amount_out).unwrap();
                require!(
                    new_debt0 <= borrowing_power1,
                    ErrorCode::BorrowingPowerExceeded
                );
            },
            false => {
                let borrowing_power0 = self.pair.total_collateral0
                    .checked_mul(self.pair.price0_mantissa())
                    .unwrap()
                    .checked_div(SCALE)
                    .unwrap()
                    .checked_div(10000)
                    .unwrap();
                
                let new_debt1 = self.pair.total_debt1.checked_add(*amount_out).unwrap();
                require!(
                    new_debt1 <= borrowing_power0,
                    ErrorCode::BorrowingPowerExceeded
                );
            }
        }
        
        Ok(())
    }

    pub fn validate_borrow_and_update(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.validate_borrow(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_borrow(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
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

        let amount_out: u64 = args.amount;

        // Transfer tokens from vault to user
        transfer_from_pool_vault_to_user(
            pair.to_account_info(),
            token_vault.to_account_info(),
            user_token_account.to_account_info(),
            vault_token_mint.to_account_info(),
            match vault_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount_out,
            vault_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // Update debt
        match user_token_account.mint == pair.token0 {
            true => {
                if pair.total_debt0_shares == 0 {
                    pair.total_debt0_shares = amount_out;
                } else {
                    let shares = amount_out
                        .checked_mul(pair.total_debt0_shares)
                        .unwrap()
                        .checked_div(pair.total_debt0)
                        .unwrap();
                    pair.total_debt0_shares = pair.total_debt0_shares.checked_add(shares).unwrap();
                }
                pair.total_debt0 = pair.total_debt0.checked_add(amount_out).unwrap();
                user_position.debt0_shares = user_position.debt0_shares.checked_add(amount_out).unwrap();
            },
            false => {
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1_shares = amount_out;
                } else {
                    let shares = amount_out
                        .checked_mul(pair.total_debt1_shares)
                        .unwrap()
                        .checked_div(pair.total_debt1)
                        .unwrap();
                    pair.total_debt1_shares = pair.total_debt1_shares.checked_add(shares).unwrap();
                }
                pair.total_debt1 = pair.total_debt1.checked_add(amount_out).unwrap();
                user_position.debt1_shares = user_position.debt1_shares.checked_add(amount_out).unwrap();
            }
        }

        // Emit event
        let (amount0, amount1) = if user_token_account.mint == pair.token0 {
            (amount_out as i64, 0)
        } else {
            (0, amount_out as i64)
        };
        
        emit!(AdjustDebtEvent {
            user: user.key(),
            amount0,
            amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
