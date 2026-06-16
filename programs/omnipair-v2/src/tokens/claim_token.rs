use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::token_interface::Mint;

use crate::{errors::ErrorCode, shared::token::is_fee_free_mint};

pub fn require_fee_free_claim_mint(mint: &InterfaceAccount<Mint>) -> Result<()> {
    require!(is_fee_free_mint(mint)?, ErrorCode::InvalidClaimMint);
    Ok(())
}

pub fn validate_claim_token_mint(
    mint: &InterfaceAccount<Mint>,
    market: Pubkey,
    asset_decimals: u8,
) -> Result<()> {
    require_fee_free_claim_mint(mint)?;
    require_eq!(mint.decimals, asset_decimals, ErrorCode::InvalidClaimMint);
    require!(
        mint.mint_authority == COption::Some(market),
        ErrorCode::InvalidClaimMint
    );
    Ok(())
}
