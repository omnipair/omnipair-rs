use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::instructions::common::require_fee_free_claim_token_mint;
use crate::{
    errors::ErrorCode,
    state::{Market, MarketAsset},
};

pub(super) fn validate_hedge_accounts<'info>(
    market: &Account<'info, Market>,
    market_asset: MarketAsset,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_token_mint: &InterfaceAccount<'info, Mint>,
    hedge_token_mint: &InterfaceAccount<'info, Mint>,
    hedge_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
    owner_hedge_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_asset)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.claim_token_mint,
        claim_token_mint.key(),
        ErrorCode::InvalidClaimMint
    );
    require_keys_eq!(
        market_side.hedge_token_mint,
        hedge_token_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.hedge_vault,
        hedge_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        hedge_vault.mint,
        claim_token_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(hedge_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_claim_account.mint,
        claim_token_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_claim_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_hedge_account.mint,
        hedge_token_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_hedge_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        hedge_token_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidMint
    );
    require_fee_free_claim_token_mint(claim_token_mint)?;
    require_fee_free_claim_token_mint(hedge_token_mint)
}
