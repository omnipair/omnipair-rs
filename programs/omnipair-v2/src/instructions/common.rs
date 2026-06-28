use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    errors::ErrorCode,
    shared::token::{
        is_fee_free_mint, is_supported_mint, is_token_2022_mint, transfer_hook_program_id,
    },
    state::{Market, MarketAsset},
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

pub fn validate_lp_mint(
    mint: &InterfaceAccount<Mint>,
    market: Pubkey,
    asset_decimals: u8,
) -> Result<()> {
    require!(is_token_2022_mint(mint)?, ErrorCode::InvalidLpMintKey);
    require!(is_fee_free_mint(mint)?, ErrorCode::InvalidLpMintKey);
    require!(
        transfer_hook_program_id(mint)? == Some(crate::ID),
        ErrorCode::InvalidLpMintKey
    );
    require_eq!(mint.decimals, asset_decimals, ErrorCode::WrongLpDecimals);
    require!(
        mint.mint_authority == COption::Some(market),
        ErrorCode::InvalidMintAuthority
    );
    require!(
        mint.freeze_authority == COption::None,
        ErrorCode::FrozenLpMint
    );
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

pub fn validate_side_vault_accounts<'info>(
    market: &Account<'info, Market>,
    market_asset: MarketAsset,
    asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_asset)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
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
    Ok(())
}

pub fn validate_owner_asset_account(
    owner: Pubkey,
    asset_mint: &InterfaceAccount<Mint>,
    owner_asset_account: &InterfaceAccount<TokenAccount>,
) -> Result<()> {
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

pub fn validate_owner_lp_account(
    owner: Pubkey,
    lp_mint: &InterfaceAccount<Mint>,
    owner_lp_account: &InterfaceAccount<TokenAccount>,
) -> Result<()> {
    require_keys_eq!(
        owner_lp_account.mint,
        lp_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_lp_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

pub fn validate_fee_accounts<'info>(
    market: &Account<'info, Market>,
    asset_mint: &InterfaceAccount<'info, Mint>,
    fee_vault: &InterfaceAccount<'info, TokenAccount>,
) -> Result<MarketAsset> {
    let market_asset = market.asset_for_mint(asset_mint.key())?;
    let market_side = market.side(market_asset)?;
    require_keys_eq!(
        market_side.fee_vault,
        fee_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(fee_vault.mint, asset_mint.key(), ErrorCode::InvalidVault);
    require_keys_eq!(fee_vault.owner, market.key(), ErrorCode::InvalidVault);
    Ok(market_asset)
}

pub fn validate_interest_accounts<'info>(
    market: &Account<'info, Market>,
    asset_mint: &InterfaceAccount<'info, Mint>,
    interest_vault: &InterfaceAccount<'info, TokenAccount>,
) -> Result<MarketAsset> {
    let market_asset = market.asset_for_mint(asset_mint.key())?;
    let market_side = market.side(market_asset)?;
    require_keys_eq!(
        market_side.interest_vault,
        interest_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        interest_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(interest_vault.owner, market.key(), ErrorCode::InvalidVault);
    Ok(market_asset)
}

pub fn validate_swap_accounts<'info>(
    market: &Account<'info, Market>,
    trader: Pubkey,
    asset_in_mint: &InterfaceAccount<'info, Mint>,
    asset_out_mint: &InterfaceAccount<'info, Mint>,
    reserve_in_vault: &InterfaceAccount<'info, TokenAccount>,
    reserve_out_vault: &InterfaceAccount<'info, TokenAccount>,
    fee_in_vault: &InterfaceAccount<'info, TokenAccount>,
    trader_asset_in_account: &InterfaceAccount<'info, TokenAccount>,
    trader_asset_out_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<MarketAsset> {
    let asset_in = market.asset_for_mint(asset_in_mint.key())?;
    let asset_out = market.asset_for_mint(asset_out_mint.key())?;
    require!(asset_out == asset_in.opposite(), ErrorCode::InvalidMint);
    let (market_side_in, market_side_out) = market.swap_sides(asset_in);
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
    validate_owner_asset_account(trader, asset_in_mint, trader_asset_in_account)?;
    validate_owner_asset_account(trader, asset_out_mint, trader_asset_out_account)?;
    Ok(asset_in)
}
