use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
    associated_token::AssociatedToken,
};
use crate::{
    state::*,
    constants::*,
    utils::token::transfer_from_vault_to_vault,
    generate_gamm_pair_seeds,
};


#[derive(Accounts)]
pub struct ClaimProtocolFees<'info> {
    /// Anyone can call this instruction
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
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
    pub reserve0_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Account<'info, TokenAccount>,

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
    pub token0_mint: Box<Account<'info, Mint>>,
    
    #[account(address = pair.token1)]
    pub token1_mint: Box<Account<'info, Mint>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimProtocolFees<'info> {
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>) -> Result<()> {
        let ClaimProtocolFees { pair, reserve0_vault, reserve1_vault, .. } = ctx.accounts;
        let claimable_amount0 = reserve0_vault.amount.saturating_sub(pair.cash_reserve0);
        let claimable_amount1 = reserve1_vault.amount.saturating_sub(pair.cash_reserve1);

        transfer_from_vault_to_vault(
            pair.to_account_info(),
            ctx.accounts.reserve0_vault.to_account_info(),
            ctx.accounts.authority_token0_account.to_account_info(),
            ctx.accounts.token0_mint.to_account_info(),
            match ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            claimable_amount0,
            ctx.accounts.token0_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        transfer_from_vault_to_vault(
            pair.to_account_info(),
            ctx.accounts.reserve1_vault.to_account_info(),
            ctx.accounts.authority_token1_account.to_account_info(),
            ctx.accounts.token1_mint.to_account_info(),
            match ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            claimable_amount1,
            ctx.accounts.token1_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        Ok(())
    }
}

