use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{Instruction, AccountMeta},
    program::invoke,
    hash::hash,
};
use anchor_spl::{
    token::{self, Token, TokenAccount, Mint},
    token_interface::{Token2022},
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    events::*,
    utils::{token::{transfer_from_vault_to_user, transfer_from_vault_to_vault, sync_native_if_wsol}, math::ceil_div},
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
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
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
        address = pair.token0 @ ErrorCode::InvalidMint
    )]
    pub token0_mint: Box<Account<'info, Mint>>,
    
    #[account(
        address = pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token1_mint: Box<Account<'info, Mint>>,

    /// Temporary repayment vault for token0. Created at instruction start, verified
    /// after callback, swept to reserve vault, then closed. Isolates repayment
    /// verification from reserve vault balance changes.
    #[account(
        init,
        payer = user,
        token::mint = token0_mint,
        token::authority = pair,
        seeds = [FLASHLOAN_REPAY_SEED_PREFIX, pair.key().as_ref(), pair.token0.as_ref()],
        bump,
    )]
    pub repay0_vault: Account<'info, TokenAccount>,

    /// Temporary repayment vault for token1.
    #[account(
        init,
        payer = user,
        token::mint = token1_mint,
        token::authority = pair,
        seeds = [FLASHLOAN_REPAY_SEED_PREFIX, pair.key().as_ref(), pair.token1.as_ref()],
        bump,
    )]
    pub repay1_vault: Account<'info, TokenAccount>,

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
    
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    
    pub system_program: Program<'info, System>,
}

impl<'info> Flashloan<'info> {
    pub fn validate(&self, args: &FlashloanArgs) -> Result<()> {
        require!(
            args.amount0 > 0 || args.amount1 > 0,
            ErrorCode::AmountZero
        );
        
        // Ensure loan amounts doesn't exceed available cash reserves
        if args.amount0 > 0 {
            require_gte!(
                self.pair.cash_reserve0,
                args.amount0,
                ErrorCode::BorrowExceedsReserve
            );
        }
        
        if args.amount1 > 0 {
            require_gte!(
                self.pair.cash_reserve1,
                args.amount1,
                ErrorCode::BorrowExceedsReserve
            );
        }
        
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;
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
            reserve0_vault,
            reserve1_vault,
            repay0_vault,
            repay1_vault,
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

        // Calculate fees (5 bps = 0.05%)
        let fee0 = ceil_div((amount0 as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(ErrorCode::FeeMathOverflow)? as u64;
        
        let fee1 = ceil_div((amount1 as u128)
            .checked_mul(FLASHLOAN_FEE_BPS as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // Transfer tokens to receiver if requested
        if amount0 > 0 {
            transfer_from_vault_to_user(
                pair.to_account_info(),
                reserve0_vault.to_account_info(),
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
            transfer_from_vault_to_user(
                pair.to_account_info(),
                reserve1_vault.to_account_info(),
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

        // Build the instruction data with Anchor discriminator
        // Anchor computes discriminators as: first 8 bytes of SHA256("global:instruction_name")
        let discriminator = &hash(b"global:flash_loan_callback").to_bytes()[..8];
        
        let mut callback_instruction_data = Vec::new();
        callback_instruction_data.extend_from_slice(discriminator);
        callback_data.serialize(&mut callback_instruction_data)?;

        // Build account metas for the CPI instruction  
        // Order must match the receiver's FlashLoanCallback account struct
        let mut callback_account_metas = vec![
            AccountMeta::new_readonly(user.key(), true),           // initiator
            AccountMeta::new(receiver_token0_account.key(), false), // receiver_token0_account
            AccountMeta::new(receiver_token1_account.key(), false), // receiver_token1_account
            AccountMeta::new_readonly(token0_mint.key(), false),    // token0_mint
            AccountMeta::new_readonly(token1_mint.key(), false),    // token1_mint
            AccountMeta::new(repay0_vault.key(), false),            // repay0_vault
            AccountMeta::new(repay1_vault.key(), false),            // repay1_vault
        ];

        // Add any additional accounts the receiver may need
        for acc in ctx.remaining_accounts.iter() {
            callback_account_metas.push(AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            });
        }
        
        // Add token_program as the last account
        callback_account_metas.push(AccountMeta::new_readonly(token_program.key(), false));

        // Build the CPI instruction to the receiver program
        let callback_instruction = Instruction {
            program_id: receiver_program.key(),
            accounts: callback_account_metas,
            data: callback_instruction_data,
        };

        // Execute the CPI callback
        let base_accounts = &[
            user.to_account_info(),
            receiver_token0_account.to_account_info(),
            receiver_token1_account.to_account_info(),
            token0_mint.to_account_info(),
            token1_mint.to_account_info(),
            repay0_vault.to_account_info(),
            repay1_vault.to_account_info(),
            token_program.to_account_info(),
        ];
        
        let all_accounts = [base_accounts, ctx.remaining_accounts].concat();

        invoke(
            &callback_instruction,
            &all_accounts,
        )?;

        // Verify repayment via isolated repay vaults (started at zero balance)
        sync_native_if_wsol(&pair.token0, &repay0_vault.to_account_info(), &token_program.to_account_info())?;
        sync_native_if_wsol(&pair.token1, &repay1_vault.to_account_info(), &token_program.to_account_info())?;

        repay0_vault.reload()?;
        repay1_vault.reload()?;

        let required0 = amount0.checked_add(fee0).ok_or(ErrorCode::FeeMathOverflow)?;
        let required1 = amount1.checked_add(fee1).ok_or(ErrorCode::FeeMathOverflow)?;

        require!(
            repay0_vault.amount >= required0,
            ErrorCode::InsufficientAmount0
        );
        require!(
            repay1_vault.amount >= required1,
            ErrorCode::InsufficientAmount1
        );

        // Sweep repaid tokens from repay vaults into reserve vaults
        let pair_seeds = generate_gamm_pair_seeds!(pair);
        let signer_seeds = &[&pair_seeds[..]];
        let repay0_amount = repay0_vault.amount;
        let repay1_amount = repay1_vault.amount;

        if repay0_amount > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                repay0_vault.to_account_info(),
                reserve0_vault.to_account_info(),
                token0_mint.to_account_info(),
                match token0_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                repay0_amount,
                token0_mint.decimals,
                signer_seeds,
            )?;
        }

        if repay1_amount > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                repay1_vault.to_account_info(),
                reserve1_vault.to_account_info(),
                token1_mint.to_account_info(),
                match token1_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                repay1_amount,
                token1_mint.decimals,
                signer_seeds,
            )?;
        }

        // Close repay vaults and return rent to user
        token::close_account(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::CloseAccount {
                    account: repay0_vault.to_account_info(),
                    destination: user.to_account_info(),
                    authority: pair.to_account_info(),
                },
                signer_seeds,
            ),
        )?;

        token::close_account(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::CloseAccount {
                    account: repay1_vault.to_account_info(),
                    destination: user.to_account_info(),
                    authority: pair.to_account_info(),
                },
                signer_seeds,
            ),
        )?;

        // Update reserves with fees
        pair.reserve0 = pair.reserve0.saturating_add(fee0);
        pair.reserve1 = pair.reserve1.saturating_add(fee1);
        pair.cash_reserve0 = pair.cash_reserve0.saturating_add(fee0);
        pair.cash_reserve1 = pair.cash_reserve1.saturating_add(fee1);

        // Emit event
        emit_cpi!(FlashloanEvent {
            amount0,
            amount1,
            fee0,
            fee1,
            receiver: receiver_program.key(),
            metadata: EventMetadata::new(user.key(), pair.key()),
        });

        Ok(())
    }
}
