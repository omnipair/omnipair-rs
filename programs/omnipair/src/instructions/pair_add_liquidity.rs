use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::token::{transfer_from_user_to_pool_vault, token_mint_to};
use crate::generate_gamm_pair_seeds;
use crate::AdjustLiquidity;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddLiquidityArgs {
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,
}

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

    pub fn validate_add_and_update(&mut self, args: &AddLiquidityArgs) -> Result<()> {
        self.validate_add(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_add(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let pair = &mut ctx.accounts.pair;
        let token0_vault = &mut ctx.accounts.token0_vault;
        let token1_vault = &mut ctx.accounts.token1_vault;
        let user_token0_account = &mut ctx.accounts.user_token0_account;
        let user_token1_account = &mut ctx.accounts.user_token1_account;
        let lp_mint = &mut ctx.accounts.lp_mint;
        let user_lp_token_account = &mut ctx.accounts.user_lp_token_account;
        let token_program = &ctx.accounts.token_program;

        // transfer token0 from user to pair
        transfer_from_user_to_pool_vault(
            ctx.accounts.user.to_account_info(),
            user_token0_account.to_account_info(),
            token0_vault.to_account_info(),
            ctx.accounts.token0_vault_mint.to_account_info(),
            match ctx.accounts.token0_vault_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            args.amount0_in,
            ctx.accounts.token0_vault_mint.decimals,
        )?;
        transfer_from_user_to_pool_vault(
            ctx.accounts.user.to_account_info(),
            user_token1_account.to_account_info(),
            token1_vault.to_account_info(),
            ctx.accounts.token1_vault_mint.to_account_info(),
            match ctx.accounts.token1_vault_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            args.amount1_in,
            ctx.accounts.token1_vault_mint.decimals,
        )?;

        // Calculate liquidity
        let total_supply = lp_mint.supply;
        let liquidity = match total_supply {
            0 => {
                let product = (args.amount0_in as u128)
                    .checked_mul(args.amount1_in as u128)
                    .ok_or(ErrorCode::Overflow)?;
                let sqrt = (product as f64).sqrt() as u128;
                sqrt.checked_sub(MIN_LIQUIDITY as u128)
                    .ok_or(ErrorCode::Overflow)?
            },
            _ => {
                let liquidity0 = (args.amount0_in as u128)
                    .checked_mul(total_supply as u128)
                    .ok_or(ErrorCode::Overflow)?
                    .checked_div(pair.reserve0 as u128)
                    .ok_or(ErrorCode::Overflow)?;
                let liquidity1 = (args.amount1_in as u128)
                    .checked_mul(total_supply as u128)
                    .ok_or(ErrorCode::Overflow)?
                    .checked_div(pair.reserve1 as u128)
                    .ok_or(ErrorCode::Overflow)?;
                liquidity0.min(liquidity1)
            }
        };
        // Check if liquidity is sufficient
        require!(
            liquidity >= args.min_liquidity_out as u128,
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
        pair.reserve0 = pair.reserve0.checked_add(args.amount0_in).unwrap();
        pair.reserve1 = pair.reserve1.checked_add(args.amount1_in).unwrap();
        pair.total_supply = pair.total_supply.checked_add(liquidity as u64).unwrap();
        
        Ok(())
    }
}
