use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount},
    associated_token::AssociatedToken,
};
use crate::state::pair::Pair;
use crate::state::rate_model::RateModel;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;

#[derive(Accounts)]
pub struct InitializePair<'info> {
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
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
    pub deployer_token0: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = token1_mint,
        token::authority = deployer,
    )]
    pub deployer_token1: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        associated_token::mint = lp_mint,
        associated_token::authority = deployer,
        payer = deployer,
        token::token_program = token_program,
    )]
    pub deployer_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

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
    pub token0_reserve_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            GAMM_RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref()
        ],
        bump,
    )]
    pub token1_reserve_vault: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            GAMM_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token0_mint.key().as_ref()
        ],
        bump,
    )]
    pub token0_collateral_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            GAMM_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref()
        ],
        bump,
    )]
    pub token1_collateral_vault: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub deployer: Signer<'info>,
    
    // system programs
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
        
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        
        let pair = &mut ctx.accounts.pair;
        pair.token0 = ctx.accounts.token0_mint.key();
        pair.token1 = ctx.accounts.token1_mint.key();
        pair.last_update = current_time;
        pair.last_rate0 = MIN_RATE;
        pair.last_rate1 = MIN_RATE;
        pair.rate_model = ctx.accounts.rate_model.key();
        
        Ok(())
    }   
}