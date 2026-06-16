use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{
    errors::ErrorCode,
    state::{Market, MarketSide},
};

pub(super) fn validate_collateral_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
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

pub(super) fn validate_borrow_accounts<'info>(
    market: &Account<'info, Market>,
    borrow_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (debt_side, collateral_side) = if borrow_asset_is_asset0 {
        (&market.side0, &market.side1)
    } else {
        (&market.side1, &market.side0)
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )?;
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    Ok(())
}

pub(super) fn validate_repay_accounts<'info>(
    market: &Account<'info, Market>,
    repay_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let debt_side = if repay_asset_is_asset0 {
        &market.side0
    } else {
        &market.side1
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )
}

fn validate_debt_reserve_accounts<'info>(
    market: &Account<'info, Market>,
    debt_side: &MarketSide,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_debt_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}
