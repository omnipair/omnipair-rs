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
    utils::account::get_size_with_discriminator,
    constants::*,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdjustPositionArgs {
    pub amount: u64,
}

/// Base accounts shared across position adjustment instructions (add collateral, remove collateral, borrow, repay).
///
/// Specific instruction contexts (e.g., AddCollateral, RemoveCollateral) compose this struct
/// and define their own handling of the UserPosition PDA separately.
/// [BaseAdjustPosition]
///    ├── AddCollateral (with init_if_needed UserPosition)
///    └── RemoveCollateral (with mut only UserPosition)
/// 
/// (due to anchor procedural macro limitations, we must duplicate critical accounts at the instruction level)
/// 
/// [Instruction Struct] 
///     ├── Critical accounts (user, pair) for seeds/payer (top level)
///     └── BaseAdjustPosition (shared vaults, tokens, programs)
#[derive(Accounts)]
pub struct BaseAdjustPosition<'info> {
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

    // #[account(
    //     init_if_needed,
    //     payer = user,
    //     space = get_size_with_discriminator::<UserPosition>(),
    //     seeds = [
    //         POSITION_SEED_PREFIX,
    //         pair.key().as_ref(),
    //         user.key().as_ref()
    //     ],
    //     bump
    // )]
    // pub user_position: Account<'info, UserPosition>,
    
    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(
        mut,
        seeds = [
            TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = pair.token0,
        token::authority = user,
    )]
    pub user_collateral_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = collateral_vault.mint)]
    pub collateral_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdjustDebt<'info> {
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
        seeds = [
            TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref()
        ],
        bump,
    )]
    pub token0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        seeds = [
            TOKEN_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref()
        ],
        bump,
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

    #[account(address = token0_vault.mint)]
    pub token0_vault_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = token1_vault.mint)]
    pub token1_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> BaseAdjustPosition<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}

impl<'info> AdjustDebt<'info> {
    // generic update function for pair internal state
    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }
}
