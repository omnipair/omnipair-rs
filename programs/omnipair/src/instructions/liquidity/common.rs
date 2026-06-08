use crate::{
    constants::*, errors::ErrorCode, state::futarchy_authority::FutarchyAuthority,
    state::pair::Pair, state::rate_model::RateModel,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint as SplMint, Token, TokenAccount as SplTokenAccount},
    token_interface::{Mint, Token2022, TokenAccount},
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
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
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
    pub reserve0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token0_account.mint == pair.token0 @ ErrorCode::InvalidTokenAccount,
        constraint = user_token0_account.owner == user.key() @ ErrorCode::InvalidTokenAccount,
    )]
    pub user_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token1_account.mint == pair.token1 @ ErrorCode::InvalidTokenAccount,
        constraint = user_token1_account.owner == user.key() @ ErrorCode::InvalidTokenAccount,
    )]
    pub user_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        address = pair.token0 @ ErrorCode::InvalidMint
    )]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        address = pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        address = pair.lp_mint @ ErrorCode::InvalidMint,
    )]
    pub lp_mint: Box<Account<'info, SplMint>>,

    #[account(
        init_if_needed,
        associated_token::mint = lp_mint,
        associated_token::authority = user,
        payer = user,
        token::token_program = token_program,
    )]
    pub user_lp_token_account: Box<Account<'info, SplTokenAccount>>,

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
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;
        Ok(())
    }
}
