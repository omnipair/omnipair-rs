use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::token::{transfer_from_vault_to_user, token_burn};
use crate::generate_gamm_pair_seeds;
use crate::liquidity::common::AdjustLiquidity;
use crate::events::{BurnEvent, UserLiquidityPositionUpdatedEvent, EventMetadata};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemoveLiquidityArgs {
    pub liquidity_in: u64,
    pub min_amount0_out: u64,
    pub min_amount1_out: u64,
}

impl<'info> AdjustLiquidity<'info> {
    fn validate_remove(&self, args: &RemoveLiquidityArgs) -> Result<()> {
        let AdjustLiquidity { 
            pair,
            user_lp_token_account,
            .. 
        } = self;

        let RemoveLiquidityArgs { 
            liquidity_in,
            ..
        } = args;

        require!(*liquidity_in > 0, ErrorCode::AmountZero);
        require!(*liquidity_in <= pair.total_supply, ErrorCode::InsufficientLiquidity);
        require!(user_lp_token_account.amount >= *liquidity_in, ErrorCode::InsufficientLiquidity);
        
        Ok(())
    }

    pub fn update_and_validate_remove(&mut self, args: &RemoveLiquidityArgs) -> Result<()> {
        self.update()?;
        self.validate_remove(args)?;
        Ok(())
    }

    pub fn handle_remove(ctx: Context<Self>, args: RemoveLiquidityArgs) -> Result<()> {
        let AdjustLiquidity {
            pair,
            user_lp_token_account,
            reserve0_vault,
            reserve1_vault,
            user_token0_account,
            user_token1_account,
            lp_mint,
            token_program,
            token_2022_program,
            token0_mint,
            token1_mint,
            ..
        } = ctx.accounts;

        // Calculate amounts to remove
        let total_supply = pair.total_supply;
        let amount0_out = (args.liquidity_in as u128)
            .checked_mul(pair.reserve0 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let amount1_out = (args.liquidity_in as u128)
            .checked_mul(pair.reserve1 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        // Check if amounts are sufficient
        require!(
            amount0_out >= args.min_amount0_out,
            ErrorCode::InsufficientLiquidity
        );
        require!(
            amount1_out >= args.min_amount1_out,
            ErrorCode::InsufficientLiquidity
        );

        // Ensure sufficient cash reserves: (internally accounted instead of relying on token account balance for deciding liquidity availability)
        // - Token account balances may include protocol fees and external donation, allowing them
        //   to be higher than the virtual reserves (r_virtual).
        // - If the invariant r_cash + r_debt = r_virtual is broken, the pool's solvency
        //   assumption (r_virtual >= r_debt) may also be violated.
        require_gte!(pair.cash_reserve0, amount0_out, ErrorCode::InsufficientCashReserve0);
        require_gte!(pair.cash_reserve1, amount1_out, ErrorCode::InsufficientCashReserve1);

        // Transfer tokens from pool to user
        transfer_from_vault_to_user(
            pair.to_account_info(),
            reserve0_vault.to_account_info(),
            user_token0_account.to_account_info(),
            token0_mint.to_account_info(),
            match token0_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount0_out,
            token0_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        transfer_from_vault_to_user(
            pair.to_account_info(),
            reserve1_vault.to_account_info(),
            user_token1_account.to_account_info(),
            token1_mint.to_account_info(),
            match token1_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount1_out,
            token1_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Reload vault accounts to get updated balances after transfers
        reserve0_vault.reload()?;
        reserve1_vault.reload()?;

        // Burn LP tokens from user
        token_burn(
            ctx.accounts.user.to_account_info(),
            token_program.to_account_info(),
            lp_mint.to_account_info(),
            user_lp_token_account.to_account_info(),
            args.liquidity_in,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Update reserves
        pair.reserve0 = pair.reserve0.checked_sub(amount0_out).ok_or(ErrorCode::ReserveUnderflow)?;
        pair.reserve1 = pair.reserve1.checked_sub(amount1_out).ok_or(ErrorCode::ReserveUnderflow)?;
        pair.total_supply = pair.total_supply.checked_sub(args.liquidity_in).ok_or(ErrorCode::SupplyUnderflow)?;

        // Update cash reserves
        pair.cash_reserve0 = pair.cash_reserve0.checked_sub(amount0_out).ok_or(ErrorCode::CashReserveUnderflow)?;
        pair.cash_reserve1 = pair.cash_reserve1.checked_sub(amount1_out).ok_or(ErrorCode::CashReserveUnderflow)?;

        // Reload LP token account to get updated balance after burn
        user_lp_token_account.reload()?;
        let user_lp_balance = user_lp_token_account.amount;
        
        // Calculate user's token amounts from LP balance (same formula as add_liquidity)
        let user_token0_amount = (user_lp_balance as u128)
            .checked_mul(pair.reserve0 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let user_token1_amount = (user_lp_balance as u128)
            .checked_mul(pair.reserve1 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        // Emit event
        emit_cpi!(BurnEvent {
            metadata: EventMetadata::new(ctx.accounts.user.key(), pair.key()),
            amount0: amount0_out,
            amount1: amount1_out,
            liquidity: args.liquidity_in,
        });

        emit_cpi!(UserLiquidityPositionUpdatedEvent {
            metadata: EventMetadata::new(ctx.accounts.user.key(), pair.key()),
            // TODO: append cash reserves to the event
            token0_amount: user_token0_amount,
            token1_amount: user_token1_amount,
            lp_amount: user_lp_balance,
            token0_mint: pair.token0,
            token1_mint: pair.token1,
            lp_mint: lp_mint.key(),
        });

        Ok(())
    }
}
