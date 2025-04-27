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
pub struct AddLiquidityArgs {
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,
}

#[derive(Accounts)]
pub struct AdjustLiquidity<'info> {
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
        mut,
        associated_token::mint = pair.token0,
        associated_token::authority = pair,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        associated_token::mint = pair.token1,
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

    #[account(
        address = token0_vault.mint
    )]
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
        token::token_program = token_program,
    )]
    pub user_lp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AdjustLiquidity<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}

