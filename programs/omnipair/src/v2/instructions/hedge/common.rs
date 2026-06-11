use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{errors::ErrorCode, v2::state::Market};
use crate::v2::instructions::common::require_fee_free_claim_mint;

pub(super) fn validate_hedge_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_mint: &InterfaceAccount<'info, Mint>,
    hedge_mint: &InterfaceAccount<'info, Mint>,
    hedge_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
    owner_hedge_account: &InterfaceAccount<'info, TokenAccount>,
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
        market_side.hedge_mint,
        hedge_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.hedge_vault,
        hedge_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(hedge_vault.mint, claim_mint.key(), ErrorCode::InvalidVault);
    require_keys_eq!(hedge_vault.owner, market.key(), ErrorCode::InvalidVault);
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
    require_keys_eq!(
        owner_hedge_account.mint,
        hedge_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_hedge_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        hedge_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidMint
    );
    require_fee_free_claim_mint(claim_mint)?;
    require_fee_free_claim_mint(hedge_mint)
}
