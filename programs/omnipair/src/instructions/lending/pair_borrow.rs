use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::ErrorCode,
    events::AdjustDebtEvent,
    utils::{token::transfer_from_pool_vault_to_user, math::{NormalizedTwoValues, normalize_two_values_to_nad}},
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

        let (
            user_collateral, 
            collateral_token_decimals, 
            user_debt
        ) = match self.user_token_account.mint == self.pair.token0 {
            true => (
                self.user_position.collateral1,
                self.pair.token1_decimals,
                self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)
            ),
            false => (
                self.user_position.collateral0,
                self.pair.token0_decimals,
                self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)
            )
        };       

        let NormalizedTwoValues { scaled_a: user_collateral_scaled, scaled_b: price_scaled } = normalize_two_values_to_nad(
            user_collateral,
            collateral_token_decimals,
            self.pair.ema_price1_nad(),
        );

        let borrowing_power = ((user_collateral_scaled as u128)
            .checked_mul(price_scaled as u128).unwrap()
            .checked_mul(CF_BPS.into()).unwrap()
            .checked_div(NAD.into()).unwrap()
            .checked_div(BPS_DENOMINATOR.into()).unwrap()) as u64;

        let new_debt = user_debt.checked_add(*borrow_amount).unwrap();
        require_gte!(
            borrowing_power,
            new_debt,
            ErrorCode::BorrowingPowerExceeded
        );
        
        Ok(())
    }

    pub fn validate_borrow_and_update(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.validate_borrow(args)?;
        self.update()?;
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
        
        // Update debt
        let (
            total_debt, 
            total_debt_shares,
            user_debt_shares
        ) = if is_token0 {
            (pair.total_debt0, &mut pair.total_debt0_shares, &mut user_position.debt0_shares)
        } else {
            (pair.total_debt1, &mut pair.total_debt1_shares, &mut user_position.debt1_shares)
        };

        // update debt shares
        *total_debt_shares = match *total_debt_shares {
            0 => borrow_amount,
            _ => {
                let shares = borrow_amount
                    .checked_mul(*total_debt_shares)
                    .unwrap()
                    .checked_div(total_debt)
                    .unwrap();
                total_debt_shares.checked_add(shares).unwrap()
            }
        };
        *user_debt_shares = user_debt_shares.checked_add(borrow_amount).unwrap();
        
        // update pair actual debt
        let new_total_debt = total_debt.checked_add(borrow_amount).unwrap();
        match is_token0 {
            true => pair.total_debt0 = new_total_debt,
            false => pair.total_debt1 = new_total_debt,
        }
        
        // Emit event
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

        Ok(())
    }
}
