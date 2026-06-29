use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{
    errors::ErrorCode,
    state::{Market, MarketAsset},
};

pub(super) fn validate_liquidation_accounts<'info>(
    market: &Account<'info, Market>,
    liquidator: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    collateral_insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    liquidator_debt_account: &InterfaceAccount<'info, TokenAccount>,
    liquidator_collateral_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<MarketAsset> {
    let debt_asset = market.asset_for_mint(debt_asset_mint.key())?;
    let (debt_side, collateral_side, insurance_vault_key, collateral_insurance_vault_key) =
        match debt_asset {
            MarketAsset::Base => (
                &market.base_side,
                &market.quote_side,
                market.insurance.base_vault,
                market.insurance.quote_vault,
            ),
            MarketAsset::Quote => (
                &market.quote_side,
                &market.base_side,
                market.insurance.quote_vault,
                market.insurance.base_vault,
            ),
        };
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault_key,
        insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_insurance_vault_key,
        collateral_insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_insurance_vault.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(insurance_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        collateral_insurance_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        liquidator_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_debt_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    Ok(debt_asset)
}
