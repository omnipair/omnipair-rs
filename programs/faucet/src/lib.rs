use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount},
    associated_token::AssociatedToken,
    token_2022::{self},
};

declare_id!("3Ckfn1LMByoDfVpcDPf7nouk5nQAUm51Zkdf1oprQTAK");

#[program]
pub mod faucet {
    use super::*;

    pub fn faucet_mint(ctx: Context<FaucetMint>) -> Result<()> {
        handle_faucet_mint(ctx)
    }
}

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
        mint::authority = faucet_authority,
    )]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        mint::authority = faucet_authority,
    )]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

fn handle_faucet_mint(ctx: Context<FaucetMint>) -> Result<()> {
    let FaucetMint {
        faucet_authority,
        user_token0_account,
        user_token1_account,
        token0_mint,
        token1_mint,
        token_program,
        ..
    } = ctx.accounts;

    // Mint 50,000 tokens to user for each token
    let mint_amount = 50_000 * 10u64.pow(6); // 50,000 * 10^6

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

/// Issue a spl_token `MintTo` instruction.
fn token_mint_to<'a>(
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_program,
            token_2022::MintTo {
                to: destination,
                authority,
                mint,
            },
            signer_seeds,
        ),
        amount,
    )
}

