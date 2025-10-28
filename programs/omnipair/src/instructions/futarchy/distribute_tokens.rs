use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_spl::token_interface::Mint;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, BPS_DENOMINATOR};
use crate::errors::ErrorCode;
use crate::generate_futarchy_authority_seeds;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DistributeTokensArgs {
    // No arguments needed - recipients and percentages are read from FutarchyAuthority
}

#[derive(Accounts)]
pub struct DistributeTokens<'info> {
    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub source_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        constraint = source_token_account.owner == futarchy_authority.key(),
        constraint = source_token_account.mint == source_mint.key(),
    )]
    pub source_token_account: Account<'info, TokenAccount>,

    #[account(mut,
        constraint = futarchy_treasury_token_account.key() == futarchy_authority.futarchy_treasury,
        constraint = futarchy_treasury_token_account.mint == source_mint.key(),
    )]
    pub futarchy_treasury_token_account: Account<'info, TokenAccount>,

    #[account(mut,
        constraint = buybacks_vault_token_account.key() == futarchy_authority.buybacks_vault,
        constraint = buybacks_vault_token_account.mint == source_mint.key(),
    )]
    pub buybacks_vault_token_account: Account<'info, TokenAccount>,

    #[account(mut,
        constraint = team_treasury_token_account.key() == futarchy_authority.team_treasury,
        constraint = team_treasury_token_account.mint == source_mint.key(),
    )]
    pub team_treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl<'info> DistributeTokens<'info> {
    pub fn validate(&self, _args: &DistributeTokensArgs) -> Result<()> {
        // Verify percentages sum to 100%
        let total_percentage = self.futarchy_authority.futarchy_treasury_percentage_bps
            .checked_add(self.futarchy_authority.buybacks_vault_percentage_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_add(self.futarchy_authority.team_treasury_percentage_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?;

        require_eq!(
            total_percentage,
            BPS_DENOMINATOR,
            ErrorCode::InvalidDistribution
        );

        Ok(())
    }

    pub fn handle_distribute(ctx: Context<Self>, _args: DistributeTokensArgs) -> Result<()> {
        let DistributeTokens {
            source_token_account,
            futarchy_treasury_token_account,
            buybacks_vault_token_account,
            team_treasury_token_account,
            token_program,
            futarchy_authority,
            ..
        } = ctx.accounts;

        // Get total balance to distribute
        let total_balance = source_token_account.amount as u128;

        // Calculate amounts for each recipient using stored percentages
        let amount1 = total_balance
            .checked_mul(futarchy_authority.futarchy_treasury_percentage_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let amount2 = total_balance
            .checked_mul(futarchy_authority.buybacks_vault_percentage_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let amount3 = total_balance
            .checked_mul(futarchy_authority.team_treasury_percentage_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // Generate PDA seeds for signing
        let seeds = generate_futarchy_authority_seeds!(futarchy_authority);

        // Transfer to recipient 1
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: source_token_account.to_account_info(),
                    to: futarchy_treasury_token_account.to_account_info(),
                    authority: futarchy_authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount1,
        )?;

        // Transfer to recipient 2
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: source_token_account.to_account_info(),
                    to: buybacks_vault_token_account.to_account_info(),
                    authority: futarchy_authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount2,
        )?;

        // Transfer to recipient 3
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: source_token_account.to_account_info(),
                    to: team_treasury_token_account.to_account_info(),
                    authority: futarchy_authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount3,
        )?;

        Ok(())
    }
}

