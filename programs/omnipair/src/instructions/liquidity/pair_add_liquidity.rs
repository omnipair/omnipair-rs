use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::token::{transfer_from_user_to_pool_vault, token_mint_to};
use crate::generate_gamm_pair_seeds;
use crate::liquidity::common::{AdjustLiquidity, AddLiquidityArgs};
use crate::utils::math::SqrtU128;
use crate::events::MintEvent;

impl<'info> AdjustLiquidity<'info> {
    fn validate_add(&self, args: &AddLiquidityArgs) -> Result<()> {
        let AdjustLiquidity { 
            user_token0_account,
            user_token1_account,
            .. 
        } = self;

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
            token0_vault,
            token1_vault,
            user_lp_token_account,
            lp_mint,
            token_program,
            token_2022_program,
            token0_vault_mint,
            token1_vault_mint,
            user,
            ..
        } = ctx.accounts;
        // transfer token0 from user to pair
        transfer_from_user_to_pool_vault(
            user.to_account_info(),
            user_token0_account.to_account_info(),
            token0_vault.to_account_info(),
            token0_vault_mint.to_account_info(),
            match token0_vault_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            args.amount0_in,
            token0_vault_mint.decimals,
        )?;
        transfer_from_user_to_pool_vault(
            user.to_account_info(),
            user_token1_account.to_account_info(),
            token1_vault.to_account_info(),
            token1_vault_mint.to_account_info(),
            match token1_vault_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            args.amount1_in,
            token1_vault_mint.decimals,
        )?;

        // Calculate liquidity
        let total_supply = lp_mint.supply;
        let liquidity: u64 = match total_supply {
            0 => {
                (args.amount0_in as u128).checked_mul(args.amount1_in as u128).ok_or(ErrorCode::LiquidityMathOverflow)?
                    .sqrt().ok_or(ErrorCode::LiquiditySqrtOverflow)?
                    .checked_sub(MIN_LIQUIDITY as u128).ok_or(ErrorCode::LiquidityUnderflow)?
                    .try_into().map_err(|_| ErrorCode::LiquidityConversionOverflow)?
            },
            _ => {
                let liquidity0 = (args.amount0_in as u128)
                    .checked_mul(total_supply as u128).ok_or(ErrorCode::LiquidityMathOverflow)?
                    .checked_div(pair.reserve0 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?;
                let liquidity1 = (args.amount1_in as u128)
                    .checked_mul(total_supply as u128).ok_or(ErrorCode::LiquidityMathOverflow)?
                    .checked_div(pair.reserve1 as u128).ok_or(ErrorCode::LiquidityMathOverflow)?;
                liquidity0.min(liquidity1).try_into().map_err(|_| ErrorCode::LiquidityConversionOverflow)?
            }
        };
        // Check if liquidity is sufficient
        require!(
            liquidity >= args.min_liquidity_out,
            ErrorCode::InsufficientLiquidity
        );
        
        // Mint LP tokens to user
        token_mint_to(
            pair.to_account_info(),
            token_program.to_account_info(),
            lp_mint.to_account_info(),
            user_lp_token_account.to_account_info(),
            liquidity as u64,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // Update reserves
        pair.reserve0 = pair.reserve0
            .checked_add(args.amount0_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.reserve1 = pair.reserve1
            .checked_add(args.amount1_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.total_supply = pair.total_supply
            .checked_add(liquidity as u64)
            .ok_or(ErrorCode::SupplyOverflow)?;
        
        // Emit event
        emit!(MintEvent {
            user: user.key(),
            amount0: args.amount0_in,
            amount1: args.amount1_in,
            liquidity: liquidity as u64,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }
}
