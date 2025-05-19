use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::{AdjustDebtEvent, UserPositionUpdatedEvent},
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
    instructions::lending::common::{CommonAdjustPosition, AdjustPositionArgs},
};

impl<'info> CommonAdjustPosition<'info> {
    pub fn validate_borrow(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount: borrow_amount } = args;
        
        require!(*borrow_amount > 0, ErrorCode::AmountZero);
        
        // Check if vault has enough tokens
        require_gte!(
            self.token_vault.amount,
            *borrow_amount,
            ErrorCode::InsufficientAmount
        );

        let user_debt = match self.user_token_account.mint == self.pair.token0 {
            true => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
            false => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
        }; 
        
        let borrow_limit = self.user_position.get_borrow_limit(&self.pair, &self.token_vault.mint);
        
        let new_debt = user_debt
            .checked_add(*borrow_amount)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        require_gte!(
            borrow_limit,
            new_debt,
            ErrorCode::BorrowingPowerExceeded
        );
        
        Ok(())
    }

    pub fn update_and_validate_borrow(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.update()?;
        self.validate_borrow(args)?;
        Ok(())
    }

    /// Handles borrowing a specific token from the AMM vault.
    ///
    /// - `vault_token_mint`: Mint address of the token the user wants to borrow.
    /// - `token_vault`: AMM liquidity vault holding the borrowable tokens (pair.token0 or pair.token1 vault).
    /// - `user_token_account`: User's associated token account that will receive the borrowed tokens.
    /// 
    /// Notes:
    /// Only the specified borrow amount of the `vault_token_mint` is transferred.
    /// Tokens are sourced directly from the AMM's liquidity vault (`token_vault`).
    /// Assumes that collateral checks have already passed via [`CommonAdjustPosition::validate_borrow`].
    pub fn handle_borrow(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
        let CommonAdjustPosition {
            token_vault,
            user_token_account,
            vault_token_mint,
            token_program,
            token_2022_program,
            user,
            user_position,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;

        let borrow_amount: u64 = args.amount;
        let is_token0 = user_token_account.mint == pair.token0;

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
            borrow_amount,
            vault_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // Update debt using the increase_debt method
        user_position.increase_debt(pair, &vault_token_mint.key(), borrow_amount)?;
        
        // Emit debt adjustment event
        let (amount0, amount1) = if is_token0 {
            (borrow_amount as i64, 0)
        } else {
            (0, borrow_amount as i64)
        };
        
        emit!(AdjustDebtEvent {
            user: user.key(),
            amount0,
            amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Emit position updated event
        emit!(UserPositionUpdatedEvent {
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
