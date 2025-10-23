use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, TransferChecked};
use anchor_spl::token_interface::Mint;

declare_id!("B8kFW5kGcjNDQ4ewQ4hAg8Mbhsv1ZMKSNKfFcVpgKPB6");

#[program]
pub mod flashloan_receiver_example {
    use super::*;

    /// Handler for Omnipair flash loan callback
    /// This is called by Omnipair's flashloan instruction via CPI
    pub fn flash_loan_callback(
        ctx: Context<FlashLoanCallback>,
        callback_data: FlashLoanCallbackData,
    ) -> Result<()> {
        let FlashLoanCallbackData {
            initiator,
            amount0,
            amount1,
            data,
        } = callback_data;

        msg!("=== Flash Loan Callback Received ===");
        msg!("Initiator: {}", initiator);
        msg!("Borrowed token0: {}", amount0);
        msg!("Borrowed token1: {}", amount1);

        if !data.is_empty() {
            msg!("Custom data length: {}", data.len());
        }

        msg!("Executing flash loan strategy...");
        
        // For this example, we just demonstrate the return flow
        // In a real implementation, you would perform arbitrage/liquidation/etc
        
        // YOUR STRATEGY GOES HERE
        // Example:
        // - Swap on DEX A
        // - Swap on DEX B
        // - Keep the profit
        
        ctx.accounts.receiver_token0_account.reload()?;
        ctx.accounts.receiver_token1_account.reload()?;
        
        require!(
            ctx.accounts.receiver_token0_account.amount >= amount0,
            FlashLoanReceiverError::InsufficientBalanceToReturn
        );
        require!(
            ctx.accounts.receiver_token1_account.amount >= amount1,
            FlashLoanReceiverError::InsufficientBalanceToReturn
        );

        msg!("Strategy complete. Returning borrowed tokens...");

        // Return token0 if borrowed
        if amount0 > 0 {
            anchor_spl::token::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.receiver_token0_account.to_account_info(),
                        mint: ctx.accounts.token0_mint.to_account_info(),
                        to: ctx.accounts.token0_vault.to_account_info(),
                        authority: ctx.accounts.initiator.to_account_info(),
                    },
                ),
                amount0,
                ctx.accounts.token0_mint.decimals,
            )?;
            msg!("✓ Returned {} of token0", amount0);
        }

        // Return token1 if borrowed
        if amount1 > 0 {
            anchor_spl::token::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.receiver_token1_account.to_account_info(),
                        mint: ctx.accounts.token1_mint.to_account_info(),
                        to: ctx.accounts.token1_vault.to_account_info(),
                        authority: ctx.accounts.initiator.to_account_info(),
                    },
                ),
                amount1,
                ctx.accounts.token1_mint.decimals,
            )?;
            msg!("✓ Returned {} of token1", amount1);
        }

        msg!("=== Flash Loan Complete ===");

        Ok(())
    }
}

/// Accounts for the flash loan callback
/// These must match the order Omnipair passes them
#[derive(Accounts)]
pub struct FlashLoanCallback<'info> {
    pub initiator: Signer<'info>,
    
    #[account(mut)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub receiver_token1_account: Account<'info, TokenAccount>,
    
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(mut)]
    pub token0_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub token1_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

/// Callback data from Omnipair
/// Must match Omnipair's FlashLoanCallbackData exactly
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FlashLoanCallbackData {
    pub initiator: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub data: Vec<u8>,
}

#[error_code]
pub enum FlashLoanReceiverError {
    #[msg("Insufficient balance to return borrowed tokens")]
    InsufficientBalanceToReturn,
}
