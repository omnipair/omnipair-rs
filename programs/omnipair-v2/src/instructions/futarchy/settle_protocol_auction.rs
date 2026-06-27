use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, ProtocolAuctionSettled},
    generate_market_seeds,
    math::{denormalize_from_nad_ceil, normalize_to_nad},
    shared::token::{is_fee_free_mint, transfer_from_user_to_vault, transfer_from_vault_to_user},
    state::{FutarchyAuthority, Market, MarketAsset, ProtocolAuctionLane},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_program_for_mint, validate_owner_asset_account,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SettleProtocolAuctionArgs {
    pub lane: ProtocolAuctionLane,
    pub sold_amount: u64,
    pub max_payment_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SettleProtocolAuctionArgs)]
pub struct SettleProtocolAuction<'info> {
    #[account(mut)]
    pub bidder: Signer<'info>,

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
        mut,
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    pub sold_mint: Box<InterfaceAccount<'info, Mint>>,
    pub accepted_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub sold_fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub bidder_payment_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub bidder_receive_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub treasury_payment_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub staking_vault_payment_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub reference_market: Box<Account<'info, Market>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> SettleProtocolAuction<'info> {
    pub fn validate(&self, args: &SettleProtocolAuctionArgs) -> Result<()> {
        self.market.assert_started()?;
        self.futarchy_authority.validate()?;
        require!(args.sold_amount > 0, ErrorCode::AmountZero);
        require!(
            args.max_payment_amount > 0,
            ErrorCode::InsufficientAuctionPayment
        );

        let auction = self.futarchy_authority.auction_config(args.lane);
        require_keys_eq!(
            self.accepted_mint.key(),
            auction.accepted_mint,
            ErrorCode::InvalidMint
        );
        require!(
            is_fee_free_mint(&self.accepted_mint)?,
            ErrorCode::InvalidMint
        );

        let sold_side = self.market.asset_for_mint(self.sold_mint.key())?;
        let market_side = self.market.side(sold_side)?;
        require_keys_eq!(
            self.sold_mint.key(),
            market_side.asset_mint,
            ErrorCode::InvalidMint
        );
        require_keys_eq!(
            self.sold_fee_vault.key(),
            market_side.fee_vault,
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            self.sold_fee_vault.mint,
            self.sold_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            self.sold_fee_vault.owner,
            self.market.key(),
            ErrorCode::InvalidVault
        );
        require_gte!(
            market_side.fees.protocol_auction_liability(args.lane),
            args.sold_amount,
            ErrorCode::UnbackedFeeLiability
        );
        require_gte!(
            self.sold_fee_vault.amount,
            args.sold_amount,
            ErrorCode::UnbackedFeeLiability
        );

        validate_owner_asset_account(
            self.bidder.key(),
            &self.accepted_mint,
            &self.bidder_payment_account,
        )?;
        validate_owner_asset_account(
            self.bidder.key(),
            &self.sold_mint,
            &self.bidder_receive_account,
        )?;
        validate_recipient_payment_account(
            &self.treasury_payment_account,
            auction.recipients.treasury,
            self.accepted_mint.key(),
        )?;
        validate_recipient_payment_account(
            &self.staking_vault_payment_account,
            auction.recipients.staking_vault,
            self.accepted_mint.key(),
        )?;
        require_supported_asset_mint(&self.sold_mint)?;
        require_supported_asset_mint(&self.accepted_mint)?;
        Ok(())
    }

    pub fn handle_settle(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: SettleProtocolAuctionArgs,
    ) -> Result<()> {
        let quote = quote_auction_settlement(&ctx.accounts, &args)?;

        transfer_auction_payment(
            &ctx.accounts,
            quote.treasury_amount,
            quote.staking_vault_amount,
        )?;
        transfer_sold_fee(&ctx.accounts, args.sold_amount)?;
        let sold_side = ctx
            .accounts
            .market
            .asset_for_mint(ctx.accounts.sold_mint.key())?;
        let (remaining_fee_liability, remaining_buyback_liability) = settle_auction_state(
            &mut ctx.accounts,
            args.lane,
            sold_side,
            args.sold_amount,
            quote.current_slot,
            quote.auction_price_nad,
        )?;
        emit_auction_settled(
            &ctx,
            &args,
            quote,
            sold_side,
            remaining_fee_liability,
            remaining_buyback_liability,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct AuctionSettlementQuote {
    current_slot: u64,
    reference_market: Pubkey,
    reference_price_nad: u64,
    auction_price_nad: u64,
    payment_amount: u64,
    treasury_amount: u64,
    staking_vault_amount: u64,
}

#[inline(never)]
fn quote_auction_settlement<'info>(
    accounts: &SettleProtocolAuction<'info>,
    args: &SettleProtocolAuctionArgs,
) -> Result<AuctionSettlementQuote> {
    let current_slot = Clock::get()?.slot;
    let auction = accounts.futarchy_authority.auction_config(args.lane);
    let (reference_market, reference_price_nad) = reference_price_nad(
        &accounts.market,
        &accounts.reference_market,
        accounts.sold_mint.key(),
        accounts.accepted_mint.key(),
        current_slot,
        auction.params.max_reference_age_slots,
    )?;
    let auction_price_nad = decayed_auction_price_nad(auction, reference_price_nad, current_slot)?;
    let payment_amount = auction_payment_amount(
        args.sold_amount,
        accounts.sold_mint.decimals,
        auction_price_nad,
        accounts.accepted_mint.decimals,
    )?;
    require!(
        payment_amount <= args.max_payment_amount,
        ErrorCode::InsufficientAuctionPayment
    );
    require_gte!(
        accounts.bidder_payment_account.amount,
        payment_amount,
        ErrorCode::InsufficientBalance
    );
    let (treasury_amount, staking_vault_amount) =
        split_payment(payment_amount, auction.recipients.staking_vault_bps)?;
    Ok(AuctionSettlementQuote {
        current_slot,
        reference_market,
        reference_price_nad,
        auction_price_nad,
        payment_amount,
        treasury_amount,
        staking_vault_amount,
    })
}

#[inline(never)]
fn emit_auction_settled<'info>(
    ctx: &Context<'_, '_, '_, 'info, SettleProtocolAuction<'info>>,
    args: &SettleProtocolAuctionArgs,
    quote: AuctionSettlementQuote,
    side: MarketAsset,
    remaining_fee_liability: u64,
    remaining_buyback_liability: u64,
) -> Result<()> {
    let market_key = ctx.accounts.market.key();
    let bidder_key = ctx.accounts.bidder.key();
    emit_cpi!(ProtocolAuctionSettled {
        market: market_key,
        reference_market: quote.reference_market,
        lane: args.lane.code(),
        side: side.code(),
        bidder: bidder_key,
        sold_mint: ctx.accounts.sold_mint.key(),
        accepted_mint: ctx.accounts.accepted_mint.key(),
        sold_amount: args.sold_amount,
        payment_amount: quote.payment_amount,
        treasury_amount: quote.treasury_amount,
        staking_vault_amount: quote.staking_vault_amount,
        reference_price_nad: quote.reference_price_nad,
        auction_price_nad: quote.auction_price_nad,
        remaining_fee_liability,
        remaining_buyback_liability,
        metadata: MarketEventMetadata::new(bidder_key, market_key)?,
    });
    Ok(())
}

#[inline(never)]
fn transfer_auction_payment<'info>(
    accounts: &SettleProtocolAuction<'info>,
    treasury_amount: u64,
    staking_vault_amount: u64,
) -> Result<()> {
    let accepted_token_program = token_program_for_mint(
        &accounts.accepted_mint,
        &accounts.token_program,
        &accounts.token_2022_program,
    )?;
    transfer_from_user_to_vault(
        accounts.bidder.to_account_info(),
        accounts.bidder_payment_account.to_account_info(),
        accounts.treasury_payment_account.to_account_info(),
        accounts.accepted_mint.to_account_info(),
        accepted_token_program.clone(),
        treasury_amount,
        accounts.accepted_mint.decimals,
    )?;
    transfer_from_user_to_vault(
        accounts.bidder.to_account_info(),
        accounts.bidder_payment_account.to_account_info(),
        accounts.staking_vault_payment_account.to_account_info(),
        accounts.accepted_mint.to_account_info(),
        accepted_token_program,
        staking_vault_amount,
        accounts.accepted_mint.decimals,
    )
}

#[inline(never)]
fn transfer_sold_fee<'info>(
    accounts: &SettleProtocolAuction<'info>,
    sold_amount: u64,
) -> Result<()> {
    let sold_token_program = token_program_for_mint(
        &accounts.sold_mint,
        &accounts.token_program,
        &accounts.token_2022_program,
    )?;
    transfer_from_vault_to_user(
        accounts.market.to_account_info(),
        accounts.sold_fee_vault.to_account_info(),
        accounts.bidder_receive_account.to_account_info(),
        accounts.sold_mint.to_account_info(),
        sold_token_program,
        sold_amount,
        accounts.sold_mint.decimals,
        &[&generate_market_seeds!(accounts.market)[..]],
    )
}

#[inline(never)]
fn settle_auction_state<'info>(
    accounts: &mut SettleProtocolAuction<'info>,
    lane: ProtocolAuctionLane,
    side: MarketAsset,
    sold_amount: u64,
    current_slot: u64,
    auction_price_nad: u64,
) -> Result<(u64, u64)> {
    accounts.sold_fee_vault.reload()?;
    let market_side = accounts.market.side_mut(side)?;
    market_side
        .fees
        .settle_protocol_auction_liability(lane, sold_amount)?;
    market_side.fees.swap_fee_vault_balance = accounts.sold_fee_vault.amount;
    market_side.fees.assert_backed()?;
    let remaining_fee_liability = market_side.fees.protocol_fee_liability;
    let remaining_buyback_liability = market_side.fees.buyback_fee_liability;

    let auction = accounts.futarchy_authority.auction_config_mut(lane);
    auction.last_settlement_slot = current_slot;
    auction.last_settlement_price_nad = auction_price_nad;
    Ok((remaining_fee_liability, remaining_buyback_liability))
}

fn validate_recipient_payment_account(
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

fn reference_price_nad(
    market: &Account<Market>,
    reference_market: &Account<Market>,
    sold_mint: Pubkey,
    accepted_mint: Pubkey,
    current_slot: u64,
    max_reference_age_slots: u64,
) -> Result<(Pubkey, u64)> {
    if sold_mint == accepted_mint {
        return Ok((market.key(), NAD));
    }
    if let Some(price_nad) = price_from_market(market, sold_mint, accepted_mint) {
        assert_fresh_reference(
            market.risk.last_snapshot_slot,
            current_slot,
            max_reference_age_slots,
        )?;
        return Ok((market.key(), price_nad));
    }

    let price_nad = price_from_market(&reference_market, sold_mint, accepted_mint)
        .ok_or(ErrorCode::InvalidMarket)?;
    assert_fresh_reference(
        reference_market.risk.last_snapshot_slot,
        current_slot,
        max_reference_age_slots,
    )?;
    Ok((reference_market.key(), price_nad))
}

fn price_from_market(market: &Market, sold_mint: Pubkey, accepted_mint: Pubkey) -> Option<u64> {
    if sold_mint == market.base_mint && accepted_mint == market.quote_mint {
        Some(market.risk.base_price_ema_nad)
    } else if sold_mint == market.quote_mint && accepted_mint == market.base_mint {
        Some(market.risk.quote_price_ema_nad)
    } else {
        None
    }
}

fn assert_fresh_reference(
    last_snapshot_slot: u64,
    current_slot: u64,
    max_reference_age_slots: u64,
) -> Result<()> {
    require!(last_snapshot_slot > 0, ErrorCode::StaleAuctionReference);
    let age = current_slot.saturating_sub(last_snapshot_slot);
    require!(
        age <= max_reference_age_slots,
        ErrorCode::StaleAuctionReference
    );
    Ok(())
}

fn decayed_auction_price_nad(
    auction: &crate::state::ProtocolAuctionConfig,
    reference_price_nad: u64,
    current_slot: u64,
) -> Result<u64> {
    require!(reference_price_nad > 0, ErrorCode::InvalidSettlementPrice);
    let start_price = (reference_price_nad as u128)
        .checked_mul(auction.params.start_multiplier_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let floor_price = (reference_price_nad as u128)
        .checked_mul(auction.params.floor_multiplier_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let elapsed_slots = current_slot
        .saturating_sub(auction.last_settlement_slot)
        .min(auction.params.duration_slots);
    let decay = start_price
        .checked_sub(floor_price)
        .ok_or(ErrorCode::MarketMathOverflow)?
        .checked_mul(elapsed_slots as u128)
        .and_then(|value| value.checked_div(auction.params.duration_slots as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let price = start_price
        .checked_sub(decay)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(price).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn auction_payment_amount(
    sold_amount: u64,
    sold_decimals: u8,
    auction_price_nad: u64,
    accepted_decimals: u8,
) -> Result<u64> {
    let sold_nad = normalize_to_nad(sold_amount as u128, sold_decimals)?;
    let payment_nad = sold_nad
        .checked_mul(auction_price_nad as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    denormalize_from_nad_ceil(payment_nad, accepted_decimals)
}

fn split_payment(payment_amount: u64, staking_vault_bps: u16) -> Result<(u64, u64)> {
    require_gte!(
        BPS_DENOMINATOR,
        staking_vault_bps,
        ErrorCode::InvalidDistribution
    );
    let staking_vault_amount = (payment_amount as u128)
        .checked_mul(staking_vault_bps as u128)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let staking_vault_amount =
        u64::try_from(staking_vault_amount).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let treasury_amount = payment_amount
        .checked_sub(staking_vault_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((treasury_amount, staking_vault_amount))
}

#[cfg(test)]
mod tests {
    include!("../../tests/instructions/futarchy/settle_protocol_auction.rs");
}
