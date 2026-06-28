use anchor_lang::prelude::*;
use anchor_spl::{
    metadata::{
        create_metadata_accounts_v3,
        mpl_token_metadata::{types::DataV2, ID as MPL_TOKEN_METADATA_PROGRAM_ID},
        CreateMetadataAccountsV3, Metadata,
    },
    token_interface::Mint,
};

use crate::{
    constants::*, errors::ErrorCode, generate_market_seeds, instructions::common::validate_lp_mint,
    state::Market,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeLpMetadataArgs {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

#[derive(Accounts)]
pub struct InitializeLpMetadata<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub market: Box<Account<'info, Market>>,

    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            METADATA_SEED_PREFIX,
            MPL_TOKEN_METADATA_PROGRAM_ID.as_ref(),
            lp_mint.key().as_ref(),
        ],
        seeds::program = MPL_TOKEN_METADATA_PROGRAM_ID,
        bump
    )]
    /// CHECK: derived/checked via seeds above.
    pub lp_token_metadata: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitializeLpMetadata<'info> {
    pub fn validate(&self, args: &InitializeLpMetadataArgs) -> Result<()> {
        validate_lp_metadata(args)?;
        let decimals = lp_decimals_for_market_mint(&self.market, self.lp_mint.key())?;
        validate_lp_mint(&self.lp_mint, self.market.key(), decimals)?;
        require_vanity_suffix(
            &self.lp_mint,
            lp_vanity_suffix(&self.market, self.lp_mint.key())?,
        )?;
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeLpMetadataArgs) -> Result<()> {
        let data = DataV2 {
            name: args.name,
            symbol: args.symbol,
            uri: args.uri,
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };

        let cpi_accounts = CreateMetadataAccountsV3 {
            metadata: ctx.accounts.lp_token_metadata.to_account_info(),
            mint: ctx.accounts.lp_mint.to_account_info(),
            mint_authority: ctx.accounts.market.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            update_authority: ctx.accounts.market.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };

        create_metadata_accounts_v3(
            CpiContext::new(
                ctx.accounts.token_metadata_program.to_account_info(),
                cpi_accounts,
            )
            .with_signer(&[&generate_market_seeds!(ctx.accounts.market)[..]]),
            data,
            true,
            true,
            None,
        )
    }
}

fn validate_lp_metadata(metadata: &InitializeLpMetadataArgs) -> Result<()> {
    require!(metadata.name.len() <= 32, ErrorCode::InvalidLpName);
    require!(metadata.name.is_ascii(), ErrorCode::InvalidLpName);
    require!(metadata.symbol.len() <= 10, ErrorCode::InvalidLpSymbol);
    require!(metadata.symbol.is_ascii(), ErrorCode::InvalidLpSymbol);
    require!(metadata.uri.len() <= 200, ErrorCode::InvalidLpUri);
    require!(metadata.uri.starts_with("http"), ErrorCode::InvalidLpUri);
    Ok(())
}

fn lp_decimals_for_market_mint(market: &Market, lp_mint: Pubkey) -> Result<u8> {
    if lp_mint == market.ylp_mint || lp_mint == market.base_side.hlp_mint {
        return Ok(market.base_side.asset_decimals);
    }
    if lp_mint == market.quote_side.hlp_mint {
        return Ok(market.quote_side.asset_decimals);
    }
    err!(ErrorCode::InvalidLpMintKey)
}

fn lp_vanity_suffix(market: &Market, lp_mint: Pubkey) -> Result<&'static str> {
    if lp_mint == market.ylp_mint {
        return Ok("yLP");
    }
    if lp_mint == market.base_side.hlp_mint || lp_mint == market.quote_side.hlp_mint {
        return Ok("hLP");
    }
    err!(ErrorCode::InvalidLpMintKey)
}

#[cfg(feature = "production")]
fn require_vanity_suffix(mint: &InterfaceAccount<Mint>, suffix: &str) -> Result<()> {
    let mint_key = mint.key().to_string();
    let start_idx = mint_key
        .len()
        .checked_sub(suffix.len())
        .ok_or(ErrorCode::InvalidLpMintKey)?;
    require_eq!(suffix, &mint_key[start_idx..], ErrorCode::InvalidLpMintKey);
    Ok(())
}

#[cfg(not(feature = "production"))]
fn require_vanity_suffix(_mint: &InterfaceAccount<Mint>, _suffix: &str) -> Result<()> {
    Ok(())
}
