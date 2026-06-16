use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketInsuranceFunded},
    shared::token::transfer_from_user_to_vault,
    state::Market,
};

use crate::instructions::common::{require_supported_asset_mint, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositInsuranceArgs {
    pub market_side_index: u8,
    pub deposit_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DepositInsuranceArgs)]
pub struct DepositInsurance<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub sponsor_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> DepositInsurance<'info> {
    pub fn validate(&self, args: &DepositInsuranceArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.sponsor_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_insurance_accounts(
            &self.market,
            args.market_side_index,
            self.sponsor.key(),
            &self.asset_mint,
            &self.insurance_vault,
            &self.sponsor_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        Ok(())
    }

    pub fn handle_deposit(ctx: Context<Self>, args: DepositInsuranceArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let sponsor_key = ctx.accounts.sponsor.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();
        let vault_balance_before = ctx.accounts.insurance_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        transfer_from_user_to_vault(
            ctx.accounts.sponsor.to_account_info(),
            ctx.accounts.sponsor_asset_account.to_account_info(),
            ctx.accounts.insurance_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.insurance_vault.reload()?;

        let insurance_credit = ctx
            .accounts
            .insurance_vault
            .amount
            .checked_sub(vault_balance_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(insurance_credit > 0, ErrorCode::AmountZero);

        if args.market_side_index == 0 {
            ctx.accounts.market.insurance_reserve.available0 = ctx
                .accounts
                .market
                .insurance_reserve
                .available0
                .checked_add(insurance_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        } else {
            ctx.accounts.market.insurance_reserve.available1 = ctx
                .accounts
                .market
                .insurance_reserve
                .available1
                .checked_add(insurance_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }

        emit_cpi!(MarketInsuranceFunded {
            market: market_key,
            sponsor: sponsor_key,
            asset_mint: asset_mint_key,
            insurance_credit,
            available0: ctx.accounts.market.insurance_reserve.available0,
            available1: ctx.accounts.market.insurance_reserve.available1,
            metadata: MarketEventMetadata::new(sponsor_key, market_key),
        });

        Ok(())
    }
}

fn validate_insurance_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    let expected_vault = if market_side_index == 0 {
        market.insurance_reserve.vault0
    } else {
        market.insurance_reserve.vault1
    };
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        expected_vault,
        insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(insurance_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_asset_account.mint,
        asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_asset_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}
