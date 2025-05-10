use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
    associated_token::AssociatedToken,
};
use crate::{
    constants::*, generate_gamm_pair_seeds, state::{pair::Pair, rate_model::RateModel}
};
use crate::errors::ErrorCode;
use crate::utils::token::{
    transfer_from_user_to_pool_vault,
    token_mint_to,  
};
use crate::instructions::liquidity::common::AddLiquidityArgs;
use crate::utils::math::SqrtU128;

#[derive(Accounts)]
pub struct BootstrapPair<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX, 
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,
    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = token0_vault_mint,
        associated_token::authority = pair,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = token1_vault_mint,
        associated_token::authority = pair,
    )]
    pub token1_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = token0_vault.mint)]
    pub token0_vault_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        address = token1_vault.mint
    )]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        mut,
        seeds = [
            LP_MINT_SEED_PREFIX,
            pair.key().as_ref(),
        ],
        bump,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        mut,
        associated_token::mint = lp_mint,
        associated_token::authority = user,
    )]
    pub user_lp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> BootstrapPair<'info> {
    pub fn validate(&self, args: &AddLiquidityArgs) -> Result<()> {
        let BootstrapPair { 
            user_token0_account,
            user_token1_account,
            .. 
        } = self;

        let AddLiquidityArgs { 
            amount0_in, 
            amount1_in, 
            .. 
        } = args;
        
        require!(!self.pair.is_initialized(), ErrorCode::PairAlreadyInitialized);
        require!(*amount0_in > 0 && *amount1_in > 0, ErrorCode::AmountZero);
        require_gte!(user_token0_account.amount, *amount0_in, ErrorCode::InsufficientAmount0In);
        require_gte!(user_token1_account.amount, *amount1_in, ErrorCode::InsufficientAmount1In);
        
        Ok(())
    }

    pub fn handle_bootstrap(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let BootstrapPair {
            pair,
            token0_vault,
            token1_vault,
            user_token0_account,
            user_token1_account,
            token_program,
            token_2022_program,
            user,
            user_lp_token_account,
            lp_mint,
            token0_vault_mint,
            token1_vault_mint,
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

        // Calculate liquidity:
        // sqrt(amount0_in * amount1_in) - MINIMUM_LIQUIDITY
        // MINIMUM_LIQUIDITY = 1000
        // 9 decimals: 1000 / 10^9 = 1e-6 full LP tokens
        // 1000 units are burned permanently.
        // This burn (~1e-6 of supply) is larger than Uniswap V2's 1e-15 burn (with 18 decimals),
        // but still negligible for users and significantly raises the cost of share inflation attacks.
        let liquidity: u64 = (args.amount0_in as u128)
            .checked_mul(args.amount1_in as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .sqrt()
            .ok_or(ErrorCode::LiquiditySqrtOverflow)?
            .checked_sub(MIN_LIQUIDITY as u128)
            .ok_or(ErrorCode::LiquidityUnderflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

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
            &[&generate_gamm_pair_seeds!(pair)[..]]
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
        
        Ok(())
    }
}
