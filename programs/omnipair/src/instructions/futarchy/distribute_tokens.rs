use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
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

    /// CHECK: Verified via constraint
    #[account(mut)]
    pub source_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub recipient1_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient2_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient3_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl<'info> DistributeTokens<'info> {
    pub fn validate(&self, _args: &DistributeTokensArgs) -> Result<()> {
        // Verify source token account is owned by FutarchyAuthority PDA
        let source_account = TokenAccount::try_deserialize(&mut &self.source_token_account.data.borrow()[..])?;
        require_eq!(
            source_account.owner,
            self.futarchy_authority.key(),
            ErrorCode::InvalidFutarchyAuthority
        );

        // Verify all recipient accounts have the same mint as source
        require_eq!(
            source_account.mint,
            self.recipient1_token_account.mint,
            ErrorCode::InvalidMint
        );
        require_eq!(
            source_account.mint,
            self.recipient2_token_account.mint,
            ErrorCode::InvalidMint
        );
        require_eq!(
            source_account.mint,
            self.recipient3_token_account.mint,
            ErrorCode::InvalidMint
        );

        // Verify percentages sum to 100%
        let total_percentage = self.futarchy_authority.recipient1_percentage_bps
            .checked_add(self.futarchy_authority.recipient2_percentage_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_add(self.futarchy_authority.recipient3_percentage_bps)
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
            recipient1_token_account,
            recipient2_token_account,
            recipient3_token_account,
            token_program,
            futarchy_authority,
            ..
        } = ctx.accounts;

        // Get total balance to distribute
        let source_account = TokenAccount::try_deserialize(&mut &source_token_account.data.borrow()[..])?;
        let total_balance = source_account.amount;

        // Calculate amounts for each recipient using stored percentages
        let amount1 = (total_balance as u128)
            .checked_mul(futarchy_authority.recipient1_percentage_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let amount2 = (total_balance as u128)
            .checked_mul(futarchy_authority.recipient2_percentage_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let amount3 = (total_balance as u128)
            .checked_mul(futarchy_authority.recipient3_percentage_bps as u128)
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
                    to: recipient1_token_account.to_account_info(),
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
                    to: recipient2_token_account.to_account_info(),
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
                    to: recipient3_token_account.to_account_info(),
                    authority: futarchy_authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount3,
        )?;

        Ok(())
    }
}

