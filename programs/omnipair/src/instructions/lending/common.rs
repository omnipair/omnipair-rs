use anchor_lang::{
    prelude::*,
    accounts::interface_account::InterfaceAccount,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    constants::*,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustCollateralArgs {
    pub amount0: u64,
    pub amount1: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustDebtArgs {
    pub amount0: u64,
    pub amount1: u64,
}

#[derive(Accounts)]
pub struct AdjustCollateral<'info> {
    #[account(
        mut,
        seeds = [
            GAMM_PAIR_SEED_PREFIX, 
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
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
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

    #[account(address = token1_vault.mint)]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct AdjustDebt<'info> {
    #[account(
        mut,
        seeds = [
            GAMM_PAIR_SEED_PREFIX, 
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
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            GAMM_TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
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

    #[account(address = token1_vault.mint)]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> AdjustCollateral<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}

impl<'info> AdjustDebt<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}
