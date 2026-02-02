use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::token::{transfer_from_user_to_vault, token_mint_to};
use crate::utils::math::ceil_div;
use crate::generate_gamm_pair_seeds;
use crate::liquidity::common::{AdjustLiquidity, AddLiquidityArgs};
use crate::events::{MintEvent, UserLiquidityPositionUpdatedEvent, EventMetadata};

impl<'info> AdjustLiquidity<'info> {
    fn validate_add(&self, args: &AddLiquidityArgs) -> Result<()> {
        let AdjustLiquidity { 
            user_token0_account,
            user_token1_account,
            futarchy_authority,
            pair,
            .. 
        } = self;

        // Check reduce-only mode (global or per-pair)
        require!(
            !futarchy_authority.is_reduce_only(pair.reduce_only),
            ErrorCode::ReduceOnlyMode
        );

        let AddLiquidityArgs { 
            amount0_in, 
            amount1_in, 
            .. 
        } = args;
        
        require!(*amount0_in > 0 && *amount1_in > 0, ErrorCode::AmountZero);
        require_gte!(user_token0_account.amount, *amount0_in, ErrorCode::InsufficientAmount0In);
        require_gte!(user_token1_account.amount, *amount1_in, ErrorCode::InsufficientAmount1In);
        
        Ok(())
    }

    pub fn update_and_validate_add(&mut self, args: &AddLiquidityArgs) -> Result<()> {
        self.update()?;
        self.validate_add(args)?;
        Ok(())
    }

    pub fn handle_add(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let AdjustLiquidity {
            pair,
            user_token0_account,
            user_token1_account,
            reserve0_vault,
            reserve1_vault,
            user_lp_token_account,
            lp_mint,
            token_program,
            token_2022_program,
            token0_mint,
            token1_mint,
            user,
            ..
        } = ctx.accounts;

        // Calculate liquidity based on input amounts
        let total_supply = pair.total_supply; // total supply is set to MIN_LIQUIDITY in initialize
        let liquidity: u64 = {
                let liquidity0 = (args.amount0_in as u128)
                    .checked_mul(total_supply as u128).ok_or(ErrorCode::LiquidityMathOverflow)?
                    .checked_div(pair.reserve0 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?;
                let liquidity1 = (args.amount1_in as u128)
                    .checked_mul(total_supply as u128).ok_or(ErrorCode::LiquidityMathOverflow)?
                    .checked_div(pair.reserve1 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?;
                liquidity0.min(liquidity1).try_into().map_err(|_| ErrorCode::LiquidityConversionOverflow)?
            };
        
        // Check if liquidity meets minimum (slippage protection)
        require!(
            liquidity >= args.min_liquidity_out,
            ErrorCode::SlippageExceeded
        );

        // Calculate exact amounts to transfer based on liquidity minted
        // amount_used = ceil(liquidity * reserve / total_supply) - round up to favor protocol
        let amount0_used: u64 = ceil_div(
            (liquidity as u128).checked_mul(pair.reserve0 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?,
            total_supply as u128
        ).ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        
        let amount1_used: u64 = ceil_div(
            (liquidity as u128).checked_mul(pair.reserve1 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?,
            total_supply as u128
        ).ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        // Transfer only the exact amounts needed
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_token0_account.to_account_info(),
            reserve0_vault.to_account_info(),
            token0_mint.to_account_info(),
            match token0_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount0_used,
            token0_mint.decimals,
        )?;
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_token1_account.to_account_info(),
            reserve1_vault.to_account_info(),
            token1_mint.to_account_info(),
            match token1_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount1_used,
            token1_mint.decimals,
        )?;
        
        // Mint LP tokens to user
        token_mint_to(
            pair.to_account_info(),
            token_program.to_account_info(),
            lp_mint.to_account_info(),
            user_lp_token_account.to_account_info(),
            liquidity as u64,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // liqudity additions equally increase both virtual and cash reserves
        // r_virtual + (amount) = r_cash + (amount) + r_debt
        // Update reserves
        pair.reserve0 = pair.reserve0
            .checked_add(amount0_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.reserve1 = pair.reserve1
            .checked_add(amount1_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.total_supply = pair.total_supply
            .checked_add(liquidity)
            .ok_or(ErrorCode::SupplyOverflow)?;

        // Update cash reserves
        pair.cash_reserve0 = pair.cash_reserve0
            .checked_add(amount0_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.cash_reserve1 = pair.cash_reserve1
            .checked_add(amount1_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        
        user_lp_token_account.reload()?;
        let user_lp_balance = user_lp_token_account.amount;
        
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
        emit_cpi!(MintEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0: amount0_used,
            amount1: amount1_used,
            liquidity: liquidity as u64,
        });

        emit_cpi!(UserLiquidityPositionUpdatedEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
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
