use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
    solana_program::{program::invoke, system_instruction}
};
use anchor_spl::{
    token::spl_token,
    token::{Token},
    token_interface::{Mint, TokenAccount, Token2022},
    associated_token::AssociatedToken,
};
use crate::state::{
    pair::Pair,
    rate_model::RateModel,
    futarchy_authority::FutarchyAuthority,
};
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;
use crate::utils::token::{
    transfer_from_user_to_pool_vault,
    token_mint_to,  
};
use crate::utils::math::SqrtU128;
use crate::events::{PairCreatedEvent, EventMetadata};
use crate::generate_gamm_pair_seeds;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAndBootstrapArgs {
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct InitializeAndBootstrap<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<Pair>(),
        seeds = [
            PAIR_SEED_PREFIX, 
            token0_mint.key().as_ref(), 
            token1_mint.key().as_ref()
            ],
        bump
    )]
    pub pair: Box<Account<'info, Pair>>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<RateModel>(),
    )]
    pub rate_model: Box<Account<'info, RateModel>>,

    #[account(
        init,
        seeds = [
            LP_MINT_SEED_PREFIX,
            pair.key().as_ref(),
        ],
        bump,
        mint::decimals = 9,
        mint::authority = pair,
        payer = deployer,
        mint::token_program = token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        associated_token::mint = lp_mint,
        associated_token::authority = deployer,
        payer = deployer,
        token::token_program = token_program,
    )]
    pub deployer_lp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = deployer,
        associated_token::mint = token0_mint,
        associated_token::authority = pair,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        init_if_needed,
        payer = deployer,
        associated_token::mint = token1_mint,
        associated_token::authority = pair,
    )]
    pub token1_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
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
        mut,
        constraint = authority_wsol_account.mint == spl_token::native_mint::id(),
        constraint = authority_wsol_account.owner == futarchy_authority.key() @ ErrorCode::InvalidFutarchyAuthority,
        constraint = *authority_wsol_account.to_account_info().owner == token_program.key() @ ErrorCode::InvalidTokenProgram,
      )]
      pub authority_wsol_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitializeAndBootstrap<'info> {
    pub fn validate(&self, args: &InitializeAndBootstrapArgs) -> Result<()> {
        let InitializeAndBootstrapArgs { 
            swap_fee_bps, 
            half_life, 
            amount0_in,
            amount1_in,
            ..
        } = args;

        // validate pool parameters
        require_gte!(BPS_DENOMINATOR, *swap_fee_bps, ErrorCode::InvalidSwapFeeBps); // 0 <= swap_fee_bps <= 100%
        require_gte!(*half_life, MIN_HALF_LIFE, ErrorCode::InvalidHalfLife); // half_life >= 1 minute
        require_gte!(MAX_HALF_LIFE, *half_life, ErrorCode::InvalidHalfLife); // half_life <= 12 hours

        // validate bootstrap parameters
        require!(*amount0_in > 0 && *amount1_in > 0, ErrorCode::AmountZero);
        require_gte!(self.deployer_token0_account.amount, *amount0_in, ErrorCode::InsufficientAmount0In);
        require_gte!(self.deployer_token1_account.amount, *amount1_in, ErrorCode::InsufficientAmount1In);

        // Enforce address of lp mint is postfixed with "omni"
        #[cfg(feature = "production")]
        {
            let token_key: String = self.lp_mint.key().to_string();
            let last_4_chars = &token_key[token_key.len() - 4..];
            require_eq!("omni", last_4_chars, ErrorCode::InvalidTokenKey);
        }
        
        Ok(())
    }

    pub fn handle_create_rate_model(&mut self) -> Result<()> {
        let rate_model = &mut self.rate_model;
        rate_model.set_inner(RateModel::new());
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeAndBootstrapArgs) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let pair = &mut ctx.accounts.pair;
        
        let InitializeAndBootstrapArgs { 
            swap_fee_bps, 
            half_life, 
            fixed_cf_bps,
            amount0_in,
            amount1_in,
            min_liquidity_out,
        } = args;

        // Collect pair creation fee from deployer to futarchy authority
        invoke(
            &system_instruction::transfer(
                ctx.accounts.deployer.key,
                &ctx.accounts.authority_wsol_account.key(),
                PAIR_CREATION_FEE_LAMPORTS,
            ),
            &[
                ctx.accounts.deployer.to_account_info(),
                ctx.accounts.authority_wsol_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        invoke(
            &spl_token::instruction::sync_native(
                ctx.accounts.token_program.key,
                &ctx.accounts.authority_wsol_account.key(),
            )?,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.authority_wsol_account.to_account_info(),
            ],
        )?;
        
        let (
            token0, 
            token1, 
            token0_decimals,
            token1_decimals,
            rate_model_key
        ) = (
            ctx.accounts.token0_mint.key(), 
            ctx.accounts.token1_mint.key(), 
            ctx.accounts.token0_mint.decimals,
            ctx.accounts.token1_mint.decimals,
            ctx.accounts.rate_model.key()
        );

        // Initialize rate model
        ctx.accounts.rate_model.set_inner(RateModel::new());

        // Initialize pair
        pair.set_inner(Pair::initialize(
            token0,
            token1,
            token0_decimals,
            token1_decimals,
            rate_model_key,
            swap_fee_bps,
            half_life,
            fixed_cf_bps,
            current_time,
            ctx.bumps.pair,
        ));

        // Transfer tokens from deployer to vaults
        transfer_from_user_to_pool_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token0_account.to_account_info(),
            ctx.accounts.token0_vault.to_account_info(),
            ctx.accounts.token0_mint.to_account_info(),
            match ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            amount0_in,
            ctx.accounts.token0_mint.decimals,
        )?;

        transfer_from_user_to_pool_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token1_account.to_account_info(),
            ctx.accounts.token1_vault.to_account_info(),
            ctx.accounts.token1_mint.to_account_info(),
            match ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            amount1_in,
            ctx.accounts.token1_mint.decimals,
        )?;

        // Calculate liquidity:
        // sqrt(amount0_in * amount1_in) - MINIMUM_LIQUIDITY
        // MINIMUM_LIQUIDITY = 1000
        // 9 decimals: 1000 / 10^9 = 1e-6 full LP tokens
        // 1000 units are burned permanently.
        // This burn (~1e-6 of supply) is larger than Uniswap V2's 1e-15 burn (with 18 decimals),
        // but still negligible for users and significantly raises the cost of share inflation attacks.
        let liquidity: u64 = (amount0_in as u128)
            .checked_mul(amount1_in as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .sqrt()
            .ok_or(ErrorCode::LiquiditySqrtOverflow)?
            .checked_sub(MIN_LIQUIDITY as u128)
            .ok_or(ErrorCode::LiquidityUnderflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        require!(
            liquidity >= min_liquidity_out,
            ErrorCode::InsufficientLiquidity
        );
        
        // Mint LP tokens to deployer
        token_mint_to(
            pair.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.deployer_lp_token_account.to_account_info(),
            liquidity,
            &[&generate_gamm_pair_seeds!(pair)[..]]
        )?;
        
        // Update reserves
        pair.reserve0 = pair.reserve0
            .checked_add(amount0_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.reserve1 = pair.reserve1
            .checked_add(amount1_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.total_supply = pair.total_supply
            .checked_add(liquidity)
            .ok_or(ErrorCode::SupplyOverflow)?;

        // Emit event
        emit_cpi!(PairCreatedEvent {
            metadata: EventMetadata::new(ctx.accounts.deployer.key(), pair.key()),
            token0: ctx.accounts.token0_mint.key(),
            token1: ctx.accounts.token1_mint.key(),
        });

        Ok(())
    }   
}
