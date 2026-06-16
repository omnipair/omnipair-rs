use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::tokens::claim_token::validate_claim_token_mint;

pub fn validate_hedge_token_mint(
    mint: &InterfaceAccount<Mint>,
    market: Pubkey,
    asset_decimals: u8,
) -> Result<()> {
    validate_claim_token_mint(mint, market, asset_decimals)
}
