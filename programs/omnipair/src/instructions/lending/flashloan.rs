use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    program::invoke,
};
use anchor_spl::{
    token::{Token, TokenAccount},
    token_interface::{Mint, Token2022},
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    events::*,
    utils::token::transfer_from_pool_vault_to_user,
    generate_gamm_pair_seeds,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FlashloanArgs {
    pub amount0: u64,
    pub amount1: u64,
    pub data: Vec<u8>,
}

/// Instruction data for the flash loan callback
/// The receiver program should expect this data format
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FlashLoanCallbackData {
    pub initiator: Pubkey,
    pub amount0: u64,
    pub amount1: u64,
    pub data: Vec<u8>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct Flashloan<'info> {
    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.key().as_ref(), pair.token1.key().as_ref()],
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
        constraint = token0_vault.mint == pair.token0,
    )]
    pub token0_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token1_vault.mint == pair.token1,
    )]
    pub token1_vault: Account<'info, TokenAccount>,

    #[account(address = token0_vault.mint)]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(address = token1_vault.mint)]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = receiver_token0_account.mint == pair.token0,
    )]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = receiver_token1_account.mint == pair.token1,
    )]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    /// CHECK: The receiver program that implements the flash loan callback
    /// This program will be invoked via CPI
    pub receiver_program: UncheckedAccount<'info>,
    
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    
    /// CHECK: System program for CPI
    pub system_program: Program<'info, System>,
}

impl<'info> Flashloan<'info> {
    pub fn validate(&self, args: &FlashloanArgs) -> Result<()> {
        require!(
            args.amount0 > 0 || args.amount1 > 0,
            ErrorCode::AmountZero
        );
        
        // Ensure loan amounts doesn't exceed available reserves
        if args.amount0 > 0 {
            require_gte!(
                self.pair.reserve0,
                args.amount0,
                ErrorCode::BorrowExceedsReserve
            );
        }
        
        if args.amount1 > 0 {
            require_gte!(
                self.pair.reserve1,
                args.amount1,
                ErrorCode::BorrowExceedsReserve
            );
        }
        
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, pair_key)?;
        Ok(())
    }

    pub fn update_and_validate(&mut self, args: &FlashloanArgs) -> Result<()> {
        self.update()?;
        self.validate(args)?;
        Ok(())
    }

    pub fn handle_flashloan(ctx: Context<'_, '_, '_, 'info, Self>, args: FlashloanArgs) -> Result<()> {
        let Flashloan {
            pair,
            token0_vault,
            token1_vault,
            receiver_token0_account,
            receiver_token1_account,
            token0_mint,
            token1_mint,
            receiver_program,
            user,
            token_program,
            token_2022_program,
            ..
        } = ctx.accounts;

        let FlashloanArgs { amount0, amount1, data } = args;

        // Record balances before the flash loan
        token0_vault.reload()?;
        token1_vault.reload()?;
        let balance0_before = token0_vault.amount;
        let balance1_before = token1_vault.amount;

        // Transfer tokens to receiver if requested
        if amount0 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token0_vault.to_account_info(),
                receiver_token0_account.to_account_info(),
                token0_mint.to_account_info(),
                match token0_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                amount0,
                token0_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        if amount1 > 0 {
            transfer_from_pool_vault_to_user(
                pair.to_account_info(),
                token1_vault.to_account_info(),
                receiver_token1_account.to_account_info(),
                token1_mint.to_account_info(),
                match token1_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                amount1,
                token1_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }
        
        // Prepare callback data
        let callback_data = FlashLoanCallbackData {
            initiator: user.key(),
            amount0,
            amount1,
            data,
        };

        // The receiver program should have a handler with discriminator for "flash_loan_callback"
        let mut callback_instruction_data = Vec::new();
        // Add discriminator (8 bytes)
        // For "flash_loan_callback", discriminator would be computed by the receiver program
        // Here we just serialize the callback data
        callback_data.serialize(&mut callback_instruction_data)?;

        // Build account metas for the CPI instruction
        let mut callback_account_metas = vec![
            // Standard accounts the callback might need
            AccountMeta::new_readonly(user.key(), true),
            AccountMeta::new(receiver_token0_account.key(), false),
            AccountMeta::new(receiver_token1_account.key(), false),
            AccountMeta::new_readonly(token0_mint.key(), false),
            AccountMeta::new_readonly(token1_mint.key(), false),
            AccountMeta::new_readonly(token_program.key(), false),
        ];

        // Add remaining accounts for callback's use
        for acc in ctx.remaining_accounts.iter() {
            callback_account_metas.push(AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            });
        }

        // Build the CPI instruction to the receiver program
        let callback_instruction = Instruction {
            program_id: receiver_program.key(),
            accounts: callback_account_metas,
            data: callback_instruction_data,
        };

        // Execute the CPI callback
        // Create a slice of base accounts, then we'll include remaining accounts
        let base_accounts = &[
            user.to_account_info(),
            receiver_token0_account.to_account_info(),
            receiver_token1_account.to_account_info(),
            token0_mint.to_account_info(),
            token1_mint.to_account_info(),
            token_program.to_account_info(),
        ];
        
        // For the CPI, we need to pass all account infos
        // Combine base accounts with remaining accounts into a single slice
        let all_accounts = [base_accounts, ctx.remaining_accounts].concat();

        invoke(
            &callback_instruction,
            &all_accounts,
        )?;

        // Reload vault accounts to get updated balances after callback execution
        token0_vault.reload()?;
        token1_vault.reload()?;

        // Verify that the balances are restored
        // The callback must have returned the tokens
        require!(
            token0_vault.amount >= balance0_before,
            ErrorCode::InsufficientAmount0
        );
        require!(
            token1_vault.amount >= balance1_before,
            ErrorCode::InsufficientAmount1
        );

        // Emit event
        emit_cpi!(FlashloanEvent {
            amount0,
            amount1,
            receiver: receiver_program.key(),
            metadata: EventMetadata::new(user.key(), pair.key()),
        });

        Ok(())
    }
}
