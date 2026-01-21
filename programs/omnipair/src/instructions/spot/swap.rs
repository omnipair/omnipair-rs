use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
    associated_token::AssociatedToken,
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    events::*,
    utils::token::{transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault},
    utils::gamm_math::CPCurve,
    utils::math::ceil_div,
    generate_gamm_pair_seeds,
};


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub amount_in: u64,
    pub min_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct Swap<'info> { 
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
        bump
    )]
    // Box used to avoid Access violation in stack frame... error
    pub pair: Box<Account<'info, Pair>>,

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
            token_in_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_in_mint.key())
    )]
    pub token_in_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_out_mint.key())
    )]
    pub token_out_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_in_account.mint == token_in_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_in_account: Account<'info, TokenAccount>,
    #[account(mut,
        constraint = user_token_out_account.mint == token_out_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_out_account: Account<'info, TokenAccount>,

    #[account(
        constraint = token_in_mint.key() == pair.token0 || token_in_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_in_mint: Box<Account<'info, Mint>>,
    #[account(
        constraint = token_out_mint.key() == pair.token0 || token_out_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_out_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = token_in_mint,
        associated_token::authority = futarchy_authority,
    )]
    pub authority_token_in_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Swap<'info> {
    pub fn validate(&self, args: &SwapArgs) -> Result<()> {
        let amount_in = args.amount_in;

        require!(amount_in > 0, ErrorCode::AmountZero);
        require_gte!(self.user_token_in_account.amount, amount_in, ErrorCode::InsufficientAmount0In);
        
        // Ensure token_in_vault and token_out_vault are different accounts
        require_keys_neq!(
            self.token_in_vault.key(),
            self.token_out_vault.key(),
            ErrorCode::InvalidVaultSameAccount
        );
        
        // Verify vaults match the correct tokens based on swap direction
        let is_token0_in = self.user_token_in_account.mint == self.pair.token0;
        
        if is_token0_in {
            // Swapping token0 -> token1
            require_keys_eq!(
                self.token_in_vault.mint,
                self.pair.token0,
                ErrorCode::InvalidTokenAccount
            );
            require_keys_eq!(
                self.token_out_vault.mint,
                self.pair.token1,
                ErrorCode::InvalidTokenAccount
            );
        } else {
            // Swapping token1 -> token0
            require_keys_eq!(
                self.token_in_vault.mint,
                self.pair.token1,
                ErrorCode::InvalidTokenAccount
            );
            require_keys_eq!(
                self.token_out_vault.mint,
                self.pair.token0,
                ErrorCode::InvalidTokenAccount
            );
        }
        
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }

    pub fn update_and_validate_swap(&mut self, args: &SwapArgs) -> Result<()> {
        self.update()?;
        self.validate(args)?;
        Ok(())
    }

    pub fn handle_swap(ctx: Context<Self>, args: SwapArgs) -> Result<()> {
        let SwapArgs { amount_in, min_amount_out } = args;
        let Swap {
            pair,
            futarchy_authority,
            token_in_vault,
            token_out_vault,
            user_token_in_account,
            user_token_out_account,
            authority_token_in_account,
            token_in_mint,
            token_out_mint,
            token_program,
            token_2022_program,
            user,
            ..        } = ctx.accounts;
        let last_k = (pair.reserve0 as u128).checked_mul(pair.reserve1 as u128).ok_or(ErrorCode::InvariantOverflow)?;
        let is_token0_in = user_token_in_account.mint == pair.token0;

        // Swap fee = LP fee + Futarchy fee
        let swap_fee = ceil_div((amount_in as u128)
            .checked_mul(pair.swap_fee_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // Calculate futarchy fee portion of the swap fee
        let futarchy_fee = ceil_div((swap_fee as u128)
            .checked_mul(futarchy_authority.revenue_share.swap_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // amount_in_after_swap_fee = amount_in - swap_fee
        let amount_in_after_swap_fee = amount_in.checked_sub(swap_fee).ok_or(ErrorCode::FeeMathOverflow)?;

        let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };

        // Δy = (Δx * y) / (x + Δx)
        let amount_out = CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_swap_fee)?;

        // Calculate the amount in with the LP portion of the fee:
        // amount_in_with_lp_fee = amount_in - swap_fee + lp_fee = amount_in - futarchy_fee
        let amount_in_with_lp_fee = amount_in.checked_sub(futarchy_fee).ok_or(ErrorCode::Overflow)?;
        let new_reserve_in = reserve_in.checked_add(amount_in_with_lp_fee).ok_or(ErrorCode::Overflow)?;
        let new_reserve_out = reserve_out.checked_sub(amount_out).ok_or(ErrorCode::Overflow)?;

        require_gte!(amount_out, min_amount_out, ErrorCode::InsufficientOutputAmount);
        // 1. r_cash >= r_out
        match is_token0_in {
            true => require_gte!(pair.cash_reserve1, amount_out, ErrorCode::InsufficientCashReserve1),
            false => require_gte!(pair.cash_reserve0, amount_out, ErrorCode::InsufficientCashReserve0),
        }

        // Update reserves
        match is_token0_in {
            true => {
                pair.reserve0 = new_reserve_in;
                pair.reserve1 = new_reserve_out;
                pair.cash_reserve0 = pair.cash_reserve0.saturating_add(amount_in_with_lp_fee);
                pair.cash_reserve1 = pair.cash_reserve1.saturating_sub(amount_out);
            },
            false => {
                pair.reserve1 = new_reserve_in;
                pair.reserve0 = new_reserve_out;
                pair.cash_reserve1 = pair.cash_reserve1.saturating_add(amount_in_with_lp_fee);
                pair.cash_reserve0 = pair.cash_reserve0.saturating_sub(amount_out);
            }
        }

        // 2. x * y >= last_k
        require_gte!((pair.reserve0 as u128).checked_mul(pair.reserve1 as u128).ok_or(ErrorCode::Overflow)?, last_k, ErrorCode::BrokenInvariant);

        // Transfer tokens
        // First: Transfer user's input tokens into the vault
        transfer_from_user_to_vault(
            user.to_account_info(),
            user_token_in_account.to_account_info(),
            token_in_vault.to_account_info(),
            token_in_mint.to_account_info(),
            match token_in_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount_in,
            token_in_mint.decimals,
        )?;

        // Second: Transfer futarchy fee from vault to authority (after user deposit ensures sufficient balance)
        if futarchy_fee > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                token_in_vault.to_account_info(),
                authority_token_in_account.to_account_info(),
                token_in_mint.to_account_info(),
                match token_in_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                futarchy_fee,
                token_in_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        // Third: Transfer output tokens to user
        transfer_from_vault_to_user(
            pair.to_account_info(),
            token_out_vault.to_account_info(),
            user_token_out_account.to_account_info(),
            token_out_mint.to_account_info(),
            match token_out_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            amount_out,
            token_out_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        
        // Emit event
        emit_cpi!(SwapEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in,
            amount_in: amount_in,
            amount_out: amount_out,
            amount_in_after_fee: amount_in_after_swap_fee as u64,
        });
        
        Ok(())
    }
}
