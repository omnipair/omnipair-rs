use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    errors::ErrorCode,
    shared::token::{is_fee_free_mint, is_supported_mint},
    v2::state::Market,
};

pub fn token_program_for_mint<'info>(
    mint: &InterfaceAccount<'info, Mint>,
    token_program: &Program<'info, Token>,
    token_2022_program: &Program<'info, Token2022>,
) -> Result<AccountInfo<'info>> {
    let mint_info = mint.to_account_info();
    if *mint_info.owner == token_program.key() {
        Ok(token_program.to_account_info())
    } else if *mint_info.owner == token_2022_program.key() {
        Ok(token_2022_program.to_account_info())
    } else {
        err!(ErrorCode::InvalidTokenProgram)
    }
}

pub fn require_supported_asset_mint(mint: &InterfaceAccount<Mint>) -> Result<()> {
    require!(is_supported_mint(mint)?, ErrorCode::InvalidTokenProgram);
    Ok(())
}

pub fn require_fee_free_claim_mint(mint: &InterfaceAccount<Mint>) -> Result<()> {
    require!(is_fee_free_mint(mint)?, ErrorCode::InvalidClaimMint);
    Ok(())
}

pub fn token_account_credit(
    balance_before: u64,
    token_account: &InterfaceAccount<TokenAccount>,
) -> Result<u64> {
    token_account
        .amount
        .checked_sub(balance_before)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub fn token_account_debit(
    balance_before: u64,
    token_account: &InterfaceAccount<TokenAccount>,
) -> Result<u64> {
    balance_before
        .checked_sub(token_account.amount)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

pub fn validate_reserve_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.claim_mint,
        claim_mint.key(),
        ErrorCode::InvalidClaimMint
    );
    require_keys_eq!(
        market_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
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
    require_keys_eq!(
        owner_claim_account.mint,
        claim_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_claim_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        claim_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidClaimMint
    );
    Ok(())
}

pub fn validate_stake_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_mint: &InterfaceAccount<'info, Mint>,
    stake_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.claim_mint,
        claim_mint.key(),
        ErrorCode::InvalidClaimMint
    );
    require_keys_eq!(
        market_side.stake_vault,
        stake_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(stake_vault.mint, claim_mint.key(), ErrorCode::InvalidVault);
    require_keys_eq!(stake_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_claim_account.mint,
        claim_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_claim_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        claim_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidClaimMint
    );
    Ok(())
}

pub fn validate_fee_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    fee_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_fee_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.fee_vault,
        fee_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(fee_vault.mint, asset_mint.key(), ErrorCode::InvalidVault);
    require_keys_eq!(fee_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_fee_account.mint,
        asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_fee_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

pub fn validate_swap_accounts<'info>(
    market: &Account<'info, Market>,
    asset_in_is_asset0: bool,
    trader: Pubkey,
    asset_in_mint: &InterfaceAccount<'info, Mint>,
    asset_out_mint: &InterfaceAccount<'info, Mint>,
    reserve_in_vault: &InterfaceAccount<'info, TokenAccount>,
    reserve_out_vault: &InterfaceAccount<'info, TokenAccount>,
    fee_in_vault: &InterfaceAccount<'info, TokenAccount>,
    trader_asset_in_account: &InterfaceAccount<'info, TokenAccount>,
    trader_asset_out_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (market_side_in, market_side_out) = if asset_in_is_asset0 {
        (&market.side0, &market.side1)
    } else {
        (&market.side1, &market.side0)
    };
    require_keys_eq!(
        market_side_in.asset_mint,
        asset_in_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side_out.asset_mint,
        asset_out_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side_in.reserve_vault,
        reserve_in_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        market_side_out.reserve_vault,
        reserve_out_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        market_side_in.fee_vault,
        fee_in_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_in_vault.mint,
        asset_in_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_out_vault.mint,
        asset_out_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        fee_in_vault.mint,
        asset_in_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_in_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_out_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(fee_in_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        trader_asset_in_account.mint,
        asset_in_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        trader_asset_in_account.owner,
        trader,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        trader_asset_out_account.mint,
        asset_out_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        trader_asset_out_account.owner,
        trader,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}
