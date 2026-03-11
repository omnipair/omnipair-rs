use anchor_lang::prelude::*;
use crate::{
    errors::ErrorCode,
    events::{AdjustDebtEvent, UserPositionUpdatedEvent, EventMetadata},
    utils::token::transfer_from_user_to_vault,
    instructions::lending::common::{CommonAdjustDebt, AdjustDebtArgs},
    state::user_position::DebtDecreaseReason,
};

impl<'info> CommonAdjustDebt<'info> {
    pub fn validate_repay(&self, args: &AdjustDebtArgs) -> Result<()> {
        let AdjustDebtArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);

        let is_repay_all = *amount == u64::MAX;
        let is_token0 = self.user_reserve_token_account.mint == self.pair.token0;
        let user_total_debt = match is_token0 {
            true => self.user_position.calculate_debt0(self.pair.total_debt0, self.pair.total_debt0_shares)?,
            false => self.user_position.calculate_debt1(self.pair.total_debt1, self.pair.total_debt1_shares)?,
        };
        let debt_to_repay = if is_repay_all { user_total_debt } else { *amount };
        
        // Check user token balance >= debt to repay
        require_gte!(
            self.user_reserve_token_account.amount,
            debt_to_repay,
            ErrorCode::InsufficientBalance
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

    pub fn update_and_validate_repay(&mut self, args: &AdjustDebtArgs) -> Result<()> {
        self.update()?;
        self.validate_repay(args)?;
        Ok(())
    }

    pub fn handle_repay(ctx: Context<Self>, args: AdjustDebtArgs) -> Result<()> {
        let CommonAdjustDebt {
            pair,
            reserve_vault,
            user_reserve_token_account,
            reserve_token_mint,
            token_program,
            token_2022_program,
            user,
            user_position,
            ..
        } = ctx.accounts;

        let is_repay_all = args.amount == u64::MAX;
        let is_token0 = user_reserve_token_account.mint == pair.token0;
        let debt_to_repay = if is_repay_all { 
            match is_token0 {
                true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
                false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
            }
        } else {
            args.amount
        };

        // Transfer tokens from user to vault
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_reserve_token_account.to_account_info(),
            reserve_vault.to_account_info(),
            reserve_token_mint.to_account_info(),
            match reserve_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            debt_to_repay,
            reserve_token_mint.decimals,
        )?;

        // Update debt
        user_position.decrease_debt(pair, &reserve_token_mint.key(), debt_to_repay, DebtDecreaseReason::Repayment)?;

        // Emit event
        let (amount0, amount1) = if user_reserve_token_account.mint == pair.token0 {
            (-(debt_to_repay as i64), 0)
        } else {
            (0, -(debt_to_repay as i64))
        };
        
        emit_cpi!(AdjustDebtEvent {
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
            collateral0_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token1),
            collateral1_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token0),
            collateral0_liquidation_cf_bps: user_position.collateral0_liquidation_cf_bps,
            collateral1_liquidation_cf_bps: user_position.collateral1_liquidation_cf_bps,
        });

        Ok(())
    }
}
