use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
    
};
use crate::utils::token::token_mint_to;
use crate::constants::PAIR_SEED_PREFIX;
use crate::state::Pair;
use crate::generate_gamm_pair_seeds;

#[derive(Accounts)]
pub struct FaucetMint<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, token0_mint.key().as_ref(), token1_mint.key().as_ref()],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        token::mint = token0_mint,
        token::authority = user,
    )]
    pub user_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = token1_mint,
        token::authority = user,
    )]
    pub user_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = token0_mint.key() == pair.token0,
    )]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = token1_mint.key() == pair.token1,
    )]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token>,
}

impl<'info> FaucetMint<'info> {
    pub fn handle_faucet_mint(ctx: Context<Self>) -> Result<()> {
        let FaucetMint {
            user,
            pair,
            user_token0_account,
            user_token1_account,
            token0_mint,
            token1_mint,
            token_program,
            ..
        } = ctx.accounts;

        // Mint 10,000 tokens to user for each token
        let mint_amount = 10_000 * 10u64.pow(6); // 10,000 tokens with 6 decimals

        // Mint Token0
        token_mint_to(
            user.to_account_info(),
            token_program.to_account_info(),
            token0_mint.to_account_info(),
            user_token0_account.to_account_info(),
            mint_amount,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Mint Token1
        token_mint_to(
            user.to_account_info(),
            token_program.to_account_info(),
            token1_mint.to_account_info(),
            user_token1_account.to_account_info(),
            mint_amount,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        Ok(())
    }
} 