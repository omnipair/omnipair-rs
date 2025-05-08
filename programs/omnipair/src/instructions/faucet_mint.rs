use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount},
    associated_token::AssociatedToken,
};
use crate::utils::token::token_mint_to;
use crate::constants::PAIR_SEED_PREFIX;
use crate::state::Pair;

#[derive(Accounts)]
pub struct FaucetMint<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: This is the faucet authority PDA that is derived from the program ID
    #[account(
        seeds = [b"faucet_authority", crate::ID.as_ref()],
        bump
    )]
    pub faucet_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, token0_mint.key().as_ref(), token1_mint.key().as_ref()],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::authority = user,
        associated_token::mint = token0_mint,
        token::token_program = token_program,
    )]
    pub user_token0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = token1_mint,
        associated_token::authority = user,
        token::token_program = token_program,
    )]
    pub user_token1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = token0_mint.key() == pair.token0,
        mint::authority = faucet_authority,
    )]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = token1_mint.key() == pair.token1,
        mint::authority = faucet_authority,
    )]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> FaucetMint<'info> {
    pub fn handle_faucet_mint(ctx: Context<Self>) -> Result<()> {
        let FaucetMint {
            faucet_authority,
            user_token0_account,
            user_token1_account,
            token0_mint,
            token1_mint,
            token_program,
            ..
        } = ctx.accounts;

        // Mint 10,000 tokens to user for each token
        let mint_amount = 50_000 * 10u64.pow(6); // 10,000 * 10^6

        let seeds = &[b"faucet_authority", crate::ID.as_ref(), &[ctx.bumps.faucet_authority]];
        let signer_seeds = &[&seeds[..]];

        // Mint Token0
        token_mint_to(
            faucet_authority.to_account_info(),
            token_program.to_account_info(),
            token0_mint.to_account_info(),
            user_token0_account.to_account_info(),
            mint_amount,
            signer_seeds,
        )?;

        // Mint Token1
        token_mint_to(
            faucet_authority.to_account_info(),
            token_program.to_account_info(),
            token1_mint.to_account_info(),
            user_token1_account.to_account_info(),
            mint_amount,
            signer_seeds,
        )?;

        Ok(())
    }
} 