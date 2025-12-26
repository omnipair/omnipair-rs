use anchor_lang::{
    prelude::*,
};
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
    associated_token::AssociatedToken,
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    state::futarchy_authority::FutarchyAuthority,
    constants::*,
    errors::ErrorCode,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddLiquidityArgs {
    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct AdjustLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX, 
            pair.token0.as_ref(),
            pair.token1.as_ref(),
            pair.params_hash.as_ref()
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
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,
    
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref(),
        ],
        bump = pair.vault_bumps.reserve0
    )]
    pub reserve0_vault: Box<Account<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Box<Account<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_token0_account: Box<Account<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token1,
        token::authority = user,
    )]
    pub user_token1_account: Box<Account<'info, TokenAccount>>,

    #[account(
        address = pair.token0 @ ErrorCode::InvalidMint
    )]
    pub token0_mint: Box<Account<'info, Mint>>,

    #[account(
        address = pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token1_mint: Box<Account<'info, Mint>>,
    
    #[account(
        mut,
        address = pair.lp_mint @ ErrorCode::InvalidMint,
    )]
    pub lp_mint: Box<Account<'info, Mint>>,
    
    #[account(
        init_if_needed,
        associated_token::mint = lp_mint,
        associated_token::authority = user,
        payer = user,
        token::token_program = token_program,
    )]
    pub user_lp_token_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> AdjustLiquidity<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }
}

