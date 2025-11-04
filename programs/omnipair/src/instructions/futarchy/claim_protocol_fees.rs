use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount},
    token_interface::{Mint, Token2022},
    associated_token::AssociatedToken,
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimProtocolFeesArgs {
    pub amount0: u64,
    pub amount1: u64,
}

#[derive(Accounts)]
pub struct ClaimProtocolFees<'info> {
    /// Anyone can call this instruction
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.pair_nonce.as_ref()],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        mut,
        constraint = token0_vault.mint == pair.token0,
    )]
    pub token0_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token1_vault.mint == pair.token1,
    )]
    pub token1_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token0_mint,
        associated_token::authority = futarchy_authority,
    )]
    pub authority_token0_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token1_mint,
        associated_token::authority = futarchy_authority,
    )]
    pub authority_token1_account: Account<'info, TokenAccount>,

    #[account(address = pair.token0)]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(address = pair.token1)]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimProtocolFees<'info> {
    pub fn validate(&self, args: &ClaimProtocolFeesArgs) -> Result<()> {
        let ClaimProtocolFeesArgs { amount0, amount1 } = args;

        require!(
            *amount0 > 0 || *amount1 > 0,
            ErrorCode::AmountZero
        );

        if *amount0 > 0 {
            require_gte!(
                self.pair.protocol_revenue_reserve0,
                *amount0,
                ErrorCode::InsufficientAmount0
            );
        }

        if *amount1 > 0 {
            require_gte!(
                self.pair.protocol_revenue_reserve1,
                *amount1,
                ErrorCode::InsufficientAmount1
            );
        }

        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>, args: ClaimProtocolFeesArgs) -> Result<()> {
        let ClaimProtocolFeesArgs { amount0, amount1 } = args;
        let pair = &mut ctx.accounts.pair;

        if amount0 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                ctx.accounts.token0_vault.to_account_info(),
                ctx.accounts.authority_token0_account.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                match ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                    true => ctx.accounts.token_program.to_account_info(),
                    false => ctx.accounts.token_2022_program.to_account_info(),
                },
                amount0,
                ctx.accounts.token0_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;

            // Deduct from protocol revenue reserve
            pair.protocol_revenue_reserve0 = pair.protocol_revenue_reserve0
                .checked_sub(amount0)
                .ok_or(ErrorCode::DebtMathOverflow)?;
        }

        if amount1 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                ctx.accounts.token1_vault.to_account_info(),
                ctx.accounts.authority_token1_account.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                match ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                    true => ctx.accounts.token_program.to_account_info(),
                    false => ctx.accounts.token_2022_program.to_account_info(),
                },
                amount1,
                ctx.accounts.token1_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;

            // Deduct from protocol revenue reserve
            pair.protocol_revenue_reserve1 = pair.protocol_revenue_reserve1
                .checked_sub(amount1)
                .ok_or(ErrorCode::DebtMathOverflow)?;
        }

        Ok(())
    }
}

