use crate::constants::*;
use crate::errors::ErrorCode;
use crate::events::{EventMetadata, MintEvent, UserLiquidityPositionUpdatedEvent};
use crate::utils::liquidity_delta_circuit_breaker::{require_top_level_liquidity_delta_ix, LiquidityDeltaInstruction};
use crate::generate_gamm_pair_seeds;
use crate::liquidity::common::{AddLiquidityArgs, AdjustLiquidity};
use crate::utils::math::ceil_div;
use crate::utils::token::{
    require_supported_mint, token_mint_to, token_program_for_mint, transfer_amounts_from_gross,
    transfer_amounts_from_net, transfer_from_user_to_vault,
};
use anchor_lang::prelude::*;

impl<'info> AdjustLiquidity<'info> {
    fn validate_add(&self, args: &AddLiquidityArgs) -> Result<()> {
        let AdjustLiquidity {
            user_token0_account,
            user_token1_account,
            futarchy_authority,
            pair,
            token0_mint,
            token1_mint,
            instructions_sysvar,
            .. 
        } = self;

        require_top_level_liquidity_delta_ix(
            &pair.key(),
            &instructions_sysvar.to_account_info(),
            LiquidityDeltaInstruction::AddLiquidity,
        )?;

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
        require_supported_mint(token0_mint)?;
        require_supported_mint(token1_mint)?;
        require_gte!(
            user_token0_account.amount,
            *amount0_in,
            ErrorCode::InsufficientAmount0In
        );
        require_gte!(
            user_token1_account.amount,
            *amount1_in,
            ErrorCode::InsufficientAmount1In
        );

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

        let token0_mint_info = token0_mint.to_account_info();
        let token1_mint_info = token1_mint.to_account_info();
        let token0_program = token_program_for_mint(
            &token0_mint_info,
            &token_program.to_account_info(),
            &token_2022_program.to_account_info(),
        )?;
        let token1_program = token_program_for_mint(
            &token1_mint_info,
            &token_program.to_account_info(),
            &token_2022_program.to_account_info(),
        )?;
        let max0 = transfer_amounts_from_gross(&token0_mint_info, args.amount0_in)?;
        let max1 = transfer_amounts_from_gross(&token1_mint_info, args.amount1_in)?;

        // Calculate liquidity based on the net amounts that can actually land in the vaults.
        let total_supply = pair.total_supply; // total supply is set to MIN_LIQUIDITY in initialize
        let liquidity: u64 = {
            let liquidity0 = (max0.net as u128)
                .checked_mul(total_supply as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?
                .checked_div(pair.reserve0 as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?;
            let liquidity1 = (max1.net as u128)
                .checked_mul(total_supply as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?
                .checked_div(pair.reserve1 as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?;
            liquidity0
                .min(liquidity1)
                .try_into()
                .map_err(|_| ErrorCode::LiquidityConversionOverflow)?
        };

        // Check if liquidity meets minimum (slippage protection)
        require!(
            liquidity >= args.min_liquidity_out,
            ErrorCode::SlippageExceeded
        );

        // Calculate exact amounts to transfer based on liquidity minted
        // amount_used = ceil(liquidity * reserve / total_supply) - round up to favor protocol
        let amount0_net_used: u64 = ceil_div(
            (liquidity as u128)
                .checked_mul(pair.reserve0 as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?,
            total_supply as u128,
        )
        .ok_or(ErrorCode::LiquidityMathOverflow)?
        .try_into()
        .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        let amount1_net_used: u64 = ceil_div(
            (liquidity as u128)
                .checked_mul(pair.reserve1 as u128)
                .ok_or(ErrorCode::LiquidityMathOverflow)?,
            total_supply as u128,
        )
        .ok_or(ErrorCode::LiquidityMathOverflow)?
        .try_into()
        .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let amount0_transfer = transfer_amounts_from_net(&token0_mint_info, amount0_net_used)?;
        let amount1_transfer = transfer_amounts_from_net(&token1_mint_info, amount1_net_used)?;
        require_gte!(
            args.amount0_in,
            amount0_transfer.gross,
            ErrorCode::InsufficientAmount0In
        );
        require_gte!(
            args.amount1_in,
            amount1_transfer.gross,
            ErrorCode::InsufficientAmount1In
        );

        // Transfer only the exact amounts needed
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_token0_account.to_account_info(),
            reserve0_vault.to_account_info(),
            token0_mint.to_account_info(),
            token0_program,
            amount0_transfer.gross,
            token0_mint.decimals,
        )?;
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_token1_account.to_account_info(),
            reserve1_vault.to_account_info(),
            token1_mint.to_account_info(),
            token1_program,
            amount1_transfer.gross,
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
        pair.reserve0 = pair
            .reserve0
            .checked_add(amount0_net_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.reserve1 = pair
            .reserve1
            .checked_add(amount1_net_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.total_supply = pair
            .total_supply
            .checked_add(liquidity)
            .ok_or(ErrorCode::SupplyOverflow)?;

        // Update cash reserves
        pair.cash_reserve0 = pair
            .cash_reserve0
            .checked_add(amount0_net_used)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.cash_reserve1 = pair
            .cash_reserve1
            .checked_add(amount1_net_used)
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
            amount0: amount0_net_used,
            amount1: amount1_net_used,
            liquidity: liquidity as u64,
        });

        emit_cpi!(UserLiquidityPositionUpdatedEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            token0_amount: user_token0_amount,
            token1_amount: user_token1_amount,
            lp_amount: user_lp_balance,
            cash_reserve0: pair.cash_reserve0,
            cash_reserve1: pair.cash_reserve1,
            token0_mint: pair.token0,
            token1_mint: pair.token1,
            lp_mint: lp_mint.key(),
        });

        Ok(())
    }
}
