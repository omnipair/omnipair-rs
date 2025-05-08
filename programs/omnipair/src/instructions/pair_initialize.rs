use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount},
    associated_token::AssociatedToken,
};
use crate::state::{
    pair::Pair,
    rate_model::RateModel,
};
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;
use crate::events::PairCreatedEvent;

#[derive(Accounts)]
pub struct InitializePair<'info> {
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
        init,
        payer = deployer,
        space = get_size_with_discriminator::<RateModel>(),
    )]
    pub rate_model:Box<Account<'info, RateModel>>,

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

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

/// TODO: add swap fee logic
impl InitializePair<'_> {
    pub fn validate(&self) -> Result<()> {
        let InitializePair { 
            token0_mint, 
            token1_mint,
            .. 
        } = self;

        // Enforce token0 < token1 to ensure unique pair addresses.
        // This prevents the same token pair from having two valid addresses (A,B) and (B,A).
        require!(
            token0_mint.key() < token1_mint.key(),
            ErrorCode::InvalidTokenOrder
        );

        // Enforce address of lp mint is postfixed with "omni"
        #[cfg(feature = "production")]
        {
            let token_key: String = self.lp_mint.key().to_string();
            let last_4_chars = &token_key[token_key.len() - 4..];
            require_eq!("omni", last_4_chars, ErrorCode::InvalidTokenKey);
        }
        
        Ok(())
    }

    pub fn handle_create(&mut self) -> Result<()> {
        let rate_model = &mut self.rate_model;
        rate_model.exp_rate = NATURAL_LOG_OF_TWO_NAD / SECONDS_PER_DAY;
        rate_model.target_util_start = TARGET_UTIL_START_NAD;
        rate_model.target_util_end = TARGET_UTIL_END_NAD;
        
        Ok(())
    }

    pub fn validate_and_create_rate_model(&mut self) -> Result<()> {
        self.validate()?;
        self.handle_create()?;
        Ok(())
    }

    // TODO: create rate model in the same instruction
    pub fn handle_initialize(ctx: Context<Self>) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let pair = &mut ctx.accounts.pair;
        
        let (
            token0, 
            token1, 
            token0_decimals,
            token1_decimals,
            rate_model
        ) = (
            ctx.accounts.token0_mint.key(), 
            ctx.accounts.token1_mint.key(), 
            ctx.accounts.token0_mint.decimals,
            ctx.accounts.token1_mint.decimals,
            ctx.accounts.rate_model.key()
        );

        pair.set_inner(Pair::initialize(
            token0,
            token1,
            token0_decimals,
            token1_decimals,
            // maybe precompute `token0_scale_to_nad` and `token1_scale_to_nad` for cheaper calculations later
            // only if token0_decimals and token1_decimals are < 9
            rate_model,
            current_time,
            ctx.bumps.pair,
        ));

        // Emit event
        emit!(PairCreatedEvent {
            token0: ctx.accounts.token0_mint.key(),
            token1: ctx.accounts.token1_mint.key(),
            pair: pair.key(),
            timestamp: current_time,
        });

        Ok(())
    }   
}