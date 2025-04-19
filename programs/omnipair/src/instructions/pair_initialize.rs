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
    generate_gamm_pair_seeds,
    state::{
        pair::Pair,
        rate_model::RateModel,
    }, U128,
};
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::{
    account::get_size_with_discriminator, 
    token::{
        transfer_from_user_to_pool_vault, 
        token_mint_to
    },
};
use crate::AddLiquidityArgs;

#[derive(Accounts)]
pub struct InitializePair<'info> {
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<Pair>(),
        seeds = [
            GAMM_PAIR_SEED_PREFIX, 
            token0_mint.key().as_ref(), 
            token1_mint.key().as_ref()
            ],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        init,
        seeds = [
            GAMM_LP_MINT_SEED_PREFIX,
            pair.key().as_ref(),
        ],
        bump,
        mint::decimals = 9,
        mint::authority = pair,
        payer = deployer,
        mint::token_program = token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    /// deployer token accounts
    #[account(
        mut,
        token::mint = token0_mint,
        token::authority = deployer,
    )]
    pub deployer_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = token1_mint,
        token::authority = deployer,
    )]
    pub deployer_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        associated_token::mint = lp_mint,
        associated_token::authority = deployer,
        payer = deployer,
        token::token_program = token_program,
    )]
    pub deployer_lp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// pair ATAs
    #[account(
        mut,
        seeds = [
            GAMM_RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token0_mint.key().as_ref()
        ],
        bump,
    )]
    pub reserve0_vault_ata: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            GAMM_RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref()
        ],
        bump,
    )]
    pub reserve1_vault_ata: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [
            GAMM_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token0_mint.key().as_ref()
        ],
        bump,
    )]
    pub collateral0_vault_ata: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            GAMM_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref()
        ],
        bump,
    )]
    pub collateral1_vault_ata: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub deployer: Signer<'info>,
    
    // system programs
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

/// TODO: add swap fee logic
impl InitializePair<'_> {
    pub fn validate(&self, args: &AddLiquidityArgs) -> Result<()> {
        let InitializePair { 
            token0_mint, 
            token1_mint,
            deployer_token0_account,
            deployer_token1_account,
            .. 
        } = self;

        let AddLiquidityArgs { 
            amount0_in, 
            amount1_in, 
            .. 
        } = args;
        
        // Enforce token0 < token1 to ensure unique pair addresses.
        // This prevents the same token pair from having two valid addresses (A,B) and (B,A).
        require!(
            token0_mint.key() < token1_mint.key(),
            ErrorCode::InvalidTokenOrder
        );

        require!(*amount0_in > 0 && *amount1_in > 0, ErrorCode::AmountZero);
        require_gte!(deployer_token0_account.amount, *amount0_in, ErrorCode::InsufficientAmount0In);
        require_gte!(deployer_token1_account.amount, *amount1_in, ErrorCode::InsufficientAmount1In);
        
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        
        let pair = &mut ctx.accounts.pair;
        pair.token0 = ctx.accounts.token0_mint.key();
        pair.token1 = ctx.accounts.token1_mint.key();
        pair.last_update = current_time;
        pair.last_rate0 = MIN_RATE;
        pair.last_rate1 = MIN_RATE;
        pair.rate_model = ctx.accounts.rate_model.key();

        let AddLiquidityArgs { 
            amount0_in, 
            amount1_in, 
            .. 
        } = args;

        // transfer token0 from deployer to pair
        transfer_from_user_to_pool_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token0_account.to_account_info(),
            ctx.accounts.reserve0_vault_ata.to_account_info(),
            ctx.accounts.token0_mint.to_account_info(), 
            match ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            args.amount0_in,
            ctx.accounts.token0_mint.decimals,
        )?;

        // transfer token1 from deployer to pair
        transfer_from_user_to_pool_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token1_account.to_account_info(),
            ctx.accounts.reserve1_vault_ata.to_account_info(),
            ctx.accounts.token1_mint.to_account_info(), 
            match ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            args.amount1_in,
            ctx.accounts.token1_mint.decimals,
        )?;

        let liquidity = U128::from(amount0_in)
        .checked_mul(U128::from(amount1_in))
        .unwrap()
        .integer_sqrt()
        .checked_sub(U128::from(MIN_LIQUIDITY))
        .unwrap()
        .as_u64();

        // mint lp tokens to deployer
        token_mint_to(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.deployer_lp_token_account.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            liquidity,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        Ok(())
    }   
}