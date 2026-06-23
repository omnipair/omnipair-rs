use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, ProtocolFeesClaimed},
    generate_market_seeds,
    shared::token::transfer_from_vault_to_user,
    state::{FutarchyAuthority, Market, RevenueDistribution},
};

use crate::instructions::common::{require_supported_asset_mint, token_program_for_mint};

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimProtocolFees<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub base_fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub quote_fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Validated against futarchy_authority.recipients.futarchy_treasury.
    #[account(address = futarchy_authority.recipients.futarchy_treasury @ ErrorCode::InvalidRecipient)]
    pub futarchy_treasury: AccountInfo<'info>,
    /// CHECK: Validated against futarchy_authority.recipients.buybacks_vault.
    #[account(address = futarchy_authority.recipients.buybacks_vault @ ErrorCode::InvalidRecipient)]
    pub buybacks_vault: AccountInfo<'info>,
    /// CHECK: Validated against futarchy_authority.recipients.team_treasury.
    #[account(address = futarchy_authority.recipients.team_treasury @ ErrorCode::InvalidRecipient)]
    pub team_treasury: AccountInfo<'info>,

    #[account(mut)]
    pub futarchy_treasury_base_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub futarchy_treasury_quote_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub buybacks_vault_base_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub buybacks_vault_quote_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub team_treasury_base_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub team_treasury_quote_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> ClaimProtocolFees<'info> {
    pub fn validate(&self) -> Result<()> {
        self.market.assert_started()?;
        self.futarchy_authority.validate()?;
        require_keys_eq!(
            self.base_mint.key(),
            self.market.base_mint,
            ErrorCode::InvalidMint
        );
        require_keys_eq!(
            self.quote_mint.key(),
            self.market.quote_mint,
            ErrorCode::InvalidMint
        );
        validate_fee_vault(
            &self.market,
            &self.base_fee_vault,
            self.market.base_side.fee_vault,
            self.base_mint.key(),
        )?;
        validate_fee_vault(
            &self.market,
            &self.quote_fee_vault,
            self.market.quote_side.fee_vault,
            self.quote_mint.key(),
        )?;
        validate_recipient_account(
            &self.futarchy_treasury_base_account,
            self.futarchy_treasury.key(),
            self.base_mint.key(),
        )?;
        validate_recipient_account(
            &self.futarchy_treasury_quote_account,
            self.futarchy_treasury.key(),
            self.quote_mint.key(),
        )?;
        validate_recipient_account(
            &self.buybacks_vault_base_account,
            self.buybacks_vault.key(),
            self.base_mint.key(),
        )?;
        validate_recipient_account(
            &self.buybacks_vault_quote_account,
            self.buybacks_vault.key(),
            self.quote_mint.key(),
        )?;
        validate_recipient_account(
            &self.team_treasury_base_account,
            self.team_treasury.key(),
            self.base_mint.key(),
        )?;
        validate_recipient_account(
            &self.team_treasury_quote_account,
            self.team_treasury.key(),
            self.quote_mint.key(),
        )?;
        require_supported_asset_mint(&self.base_mint)?;
        require_supported_asset_mint(&self.quote_mint)?;
        Ok(())
    }

    pub fn handle_claim(mut ctx: Context<Self>) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let caller_key = ctx.accounts.caller.key();
        let base_mint_key = ctx.accounts.base_mint.key();
        let quote_mint_key = ctx.accounts.quote_mint.key();
        let base_protocol_fee = ctx.accounts.market.base_side.fees.protocol_fee_liability;
        let quote_protocol_fee = ctx.accounts.market.quote_side.fees.protocol_fee_liability;
        require!(
            base_protocol_fee > 0 || quote_protocol_fee > 0,
            ErrorCode::AmountZero
        );
        require_gte!(
            ctx.accounts.base_fee_vault.amount,
            base_protocol_fee,
            ErrorCode::UnbackedFeeLiability
        );
        require_gte!(
            ctx.accounts.quote_fee_vault.amount,
            quote_protocol_fee,
            ErrorCode::UnbackedFeeLiability
        );

        let base_split = split_protocol_fee(
            base_protocol_fee,
            &ctx.accounts.futarchy_authority.revenue_distribution,
        )?;
        let quote_split = split_protocol_fee(
            quote_protocol_fee,
            &ctx.accounts.futarchy_authority.revenue_distribution,
        )?;

        transfer_protocol_split(&mut ctx, true, base_split)?;
        transfer_protocol_split(&mut ctx, false, quote_split)?;

        ctx.accounts.base_fee_vault.reload()?;
        ctx.accounts.quote_fee_vault.reload()?;
        ctx.accounts.market.base_side.fees.protocol_fee_liability = 0;
        ctx.accounts.market.quote_side.fees.protocol_fee_liability = 0;
        ctx.accounts.market.base_side.fees.swap_fee_vault_balance =
            ctx.accounts.base_fee_vault.amount;
        ctx.accounts.market.quote_side.fees.swap_fee_vault_balance =
            ctx.accounts.quote_fee_vault.amount;
        ctx.accounts.market.base_side.fees.assert_backed()?;
        ctx.accounts.market.quote_side.fees.assert_backed()?;

        emit_cpi!(ProtocolFeesClaimed {
            market: market_key,
            base_mint: base_mint_key,
            quote_mint: quote_mint_key,
            futarchy_treasury_base_amount: base_split.futarchy_treasury_amount,
            futarchy_treasury_quote_amount: quote_split.futarchy_treasury_amount,
            buybacks_vault_base_amount: base_split.buybacks_vault_amount,
            buybacks_vault_quote_amount: quote_split.buybacks_vault_amount,
            team_treasury_base_amount: base_split.team_treasury_amount,
            team_treasury_quote_amount: quote_split.team_treasury_amount,
            metadata: MarketEventMetadata::new(caller_key, market_key)?,
        });

        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ProtocolFeeSplit {
    futarchy_treasury_amount: u64,
    buybacks_vault_amount: u64,
    team_treasury_amount: u64,
}

fn split_protocol_fee(
    fee_amount: u64,
    distribution: &RevenueDistribution,
) -> Result<ProtocolFeeSplit> {
    let buybacks_vault_amount = (fee_amount as u128)
        .checked_mul(distribution.buybacks_vault_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::FeeMathOverflow)?;
    let buybacks_vault_amount =
        u64::try_from(buybacks_vault_amount).map_err(|_| ErrorCode::FeeMathOverflow)?;
    let team_treasury_amount = (fee_amount as u128)
        .checked_mul(distribution.team_treasury_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::FeeMathOverflow)?;
    let team_treasury_amount =
        u64::try_from(team_treasury_amount).map_err(|_| ErrorCode::FeeMathOverflow)?;
    let futarchy_treasury_amount = fee_amount
        .saturating_sub(buybacks_vault_amount)
        .saturating_sub(team_treasury_amount);
    Ok(ProtocolFeeSplit {
        futarchy_treasury_amount,
        buybacks_vault_amount,
        team_treasury_amount,
    })
}

fn validate_fee_vault(
    market: &Account<Market>,
    fee_vault: &InterfaceAccount<TokenAccount>,
    expected_vault: Pubkey,
    expected_mint: Pubkey,
) -> Result<()> {
    require_keys_eq!(fee_vault.key(), expected_vault, ErrorCode::InvalidVault);
    require_keys_eq!(fee_vault.mint, expected_mint, ErrorCode::InvalidVault);
    require_keys_eq!(fee_vault.owner, market.key(), ErrorCode::InvalidVault);
    Ok(())
}

fn validate_recipient_account(
    token_account: &InterfaceAccount<TokenAccount>,
    expected_owner: Pubkey,
    expected_mint: Pubkey,
) -> Result<()> {
    require_keys_eq!(
        token_account.owner,
        expected_owner,
        ErrorCode::InvalidRecipient
    );
    require_keys_eq!(token_account.mint, expected_mint, ErrorCode::InvalidMint);
    Ok(())
}

fn transfer_protocol_split<'info>(
    ctx: &mut Context<ClaimProtocolFees<'info>>,
    base_side: bool,
    split: ProtocolFeeSplit,
) -> Result<()> {
    if base_side {
        let token_program = token_program_for_mint(
            &ctx.accounts.base_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_split_to_recipients(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.base_fee_vault.to_account_info(),
            ctx.accounts.base_mint.to_account_info(),
            token_program,
            ctx.accounts.base_mint.decimals,
            split,
            ctx.accounts
                .futarchy_treasury_base_account
                .to_account_info(),
            ctx.accounts.buybacks_vault_base_account.to_account_info(),
            ctx.accounts.team_treasury_base_account.to_account_info(),
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )
    } else {
        let token_program = token_program_for_mint(
            &ctx.accounts.quote_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_split_to_recipients(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.quote_fee_vault.to_account_info(),
            ctx.accounts.quote_mint.to_account_info(),
            token_program,
            ctx.accounts.quote_mint.decimals,
            split,
            ctx.accounts
                .futarchy_treasury_quote_account
                .to_account_info(),
            ctx.accounts.buybacks_vault_quote_account.to_account_info(),
            ctx.accounts.team_treasury_quote_account.to_account_info(),
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn transfer_split_to_recipients<'info>(
    authority: AccountInfo<'info>,
    source_vault: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    decimals: u8,
    split: ProtocolFeeSplit,
    futarchy_treasury_account: AccountInfo<'info>,
    buybacks_vault_account: AccountInfo<'info>,
    team_treasury_account: AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    transfer_from_vault_to_user(
        authority.clone(),
        source_vault.clone(),
        futarchy_treasury_account,
        mint.clone(),
        token_program.clone(),
        split.futarchy_treasury_amount,
        decimals,
        signer_seeds,
    )?;
    transfer_from_vault_to_user(
        authority.clone(),
        source_vault.clone(),
        buybacks_vault_account,
        mint.clone(),
        token_program.clone(),
        split.buybacks_vault_amount,
        decimals,
        signer_seeds,
    )?;
    transfer_from_vault_to_user(
        authority,
        source_vault,
        team_treasury_account,
        mint,
        token_program,
        split.team_treasury_amount,
        decimals,
        signer_seeds,
    )
}
