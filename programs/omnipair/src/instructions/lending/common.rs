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
    state::user_position::UserPosition,
    constants::*,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustPositionArgs {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct CommonAdjustPosition<'info> {
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
        mut,
        constraint = token_vault.mint == pair.token0 || token_vault.mint == pair.token1,
    )]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_account.mint == pair.token0 || user_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = token_vault.mint)]
    pub vault_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> CommonAdjustPosition<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}