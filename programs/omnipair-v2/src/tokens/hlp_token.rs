//! Validation helpers for V2 hLP mints.
//!
//! hLP tokens are aggregate hedged LP vault shares. They are not wrappers over
//! a single yLP side; each hLP vault owns both yLP sides and underlying debt.

use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::token_interface::Mint;

use crate::{
    errors::ErrorCode,
    shared::token::{is_fee_free_mint, is_token_2022_mint, transfer_hook_program_id},
};

pub fn validate_hlp_mint(
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
    Ok(())
}
