use anchor_lang::{
    prelude::*,
};
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    state::user_position::UserPosition,
    state::futarchy_authority::FutarchyAuthority,
    constants::*,
    errors::ErrorCode,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustCollateralArgs {
    pub amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct CommonAdjustCollateral<'info> {
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
        constraint = user_position.owner == user.key(),
        constraint = user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

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
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = match collateral_vault.mint == pair.token0 {
            true => pair.vault_bumps.collateral0,
            false => pair.vault_bumps.collateral1
        }
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_collateral_token_account.mint == pair.token0 || user_collateral_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_collateral_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> CommonAdjustCollateral<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustDebtArgs {
    pub amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct CommonAdjustDebt<'info> {
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
        constraint = user_position.owner == user.key(),
        constraint = user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

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
            reserve_token_mint.key().as_ref(),
        ],
        bump = match reserve_vault.mint == pair.token0 {
            true => pair.vault_bumps.reserve0,
            false => pair.vault_bumps.reserve1
        }
    )]
    pub reserve_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_reserve_token_account.mint == pair.token0 || user_reserve_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_reserve_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = reserve_token_mint.key() == pair.token0 || reserve_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub reserve_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> CommonAdjustDebt<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }
}