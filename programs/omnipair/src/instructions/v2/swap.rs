use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadataV2, MarketSwapV2},
    generate_market_v2_seeds,
    state::{MarketSideV2, MarketV2},
    utils::{
        gamm_math::CPCurve,
        market_v2_math::require_market_reserve_floor,
        math::ceil_div,
        token::{
            transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
        },
    },
};

use super::common::{require_supported_asset_mint, token_program_for_mint, validate_swap_accounts};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapV2Args {
    pub asset_in_is_asset0: bool,
    pub exact_asset_in: u64,
    pub min_asset_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SwapV2Args)]
pub struct SwapV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Account<'info, MarketV2>,

    #[account(mut)]
    pub trader: Signer<'info>,

    pub asset_in_mint: InterfaceAccount<'info, Mint>,

    pub asset_out_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub reserve_in_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub reserve_out_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub fee_in_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub trader_asset_in_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub trader_asset_out_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> SwapV2<'info> {
    pub fn validate(&self, args: &SwapV2Args) -> Result<()> {
        self.market.assert_live()?;
        require!(args.exact_asset_in > 0, ErrorCode::AmountZero);
        require_gte!(
            self.trader_asset_in_account.amount,
            args.exact_asset_in,
            ErrorCode::InsufficientBalance
        );
        validate_swap_accounts(
            &self.market,
            args.asset_in_is_asset0,
            self.trader.key(),
            &self.asset_in_mint,
            &self.asset_out_mint,
            &self.reserve_in_vault,
            &self.reserve_out_vault,
            &self.fee_in_vault,
            &self.trader_asset_in_account,
            &self.trader_asset_out_account,
        )?;
        require_supported_asset_mint(&self.asset_in_mint)?;
        require_supported_asset_mint(&self.asset_out_mint)?;
        Ok(())
    }

    pub fn handle_swap(mut ctx: Context<Self>, args: SwapV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let trader_key = ctx.accounts.trader.key();
        let asset_in_mint_key = ctx.accounts.asset_in_mint.key();
        let asset_out_mint_key = ctx.accounts.asset_out_mint.key();
        let operator_fee_bps = ctx.accounts.market.config.operator_fee_bps;

        let reserve_credit = receive_swap_inventory(&mut ctx, args.exact_asset_in)?;
        let total_fee = ceil_div(
            (reserve_credit as u128)
                .checked_mul(ctx.accounts.market.config.swap_fee_bps as u128)
                .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::FeeMathOverflow)?
        .min(reserve_credit as u128) as u64;

        let fee_credit = move_swap_fee(&mut ctx, total_fee)?;
        let amount_in_after_fee = reserve_credit
            .checked_sub(total_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require!(amount_in_after_fee > 0, ErrorCode::InsufficientOutputAmount);

        let amount_out = {
            let (market_side_in, market_side_out) =
                ctx.accounts.market.swap_sides(args.asset_in_is_asset0);
            CPCurve::calculate_amount_out(
                market_side_in.reserve_ledger.live_reserve,
                market_side_out.reserve_ledger.live_reserve,
                amount_in_after_fee,
            )?
        };
        require_gte!(amount_out, args.min_asset_out, ErrorCode::SlippageExceeded);

        let asset_out_token_program = token_program_for_mint(
            &ctx.accounts.asset_out_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_out_vault.to_account_info(),
            ctx.accounts.trader_asset_out_account.to_account_info(),
            ctx.accounts.asset_out_mint.to_account_info(),
            asset_out_token_program,
            amount_out,
            ctx.accounts.asset_out_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;

        {
            let (market_side_in, market_side_out) =
                ctx.accounts.market.swap_sides_mut(args.asset_in_is_asset0);
            apply_swap_state(
                market_side_in,
                market_side_out,
                amount_in_after_fee,
                amount_out,
                fee_credit,
                operator_fee_bps,
            )?;
        }

        emit_cpi!(MarketSwapV2 {
            market: market_key,
            trader: trader_key,
            asset_in_mint: asset_in_mint_key,
            asset_out_mint: asset_out_mint_key,
            reserve_credit,
            amount_in_after_fee,
            amount_out,
            fee_credit,
            metadata: MarketEventMetadataV2::new(trader_key, market_key),
        });

        Ok(())
    }
}

fn receive_swap_inventory<'info>(
    ctx: &mut Context<SwapV2<'info>>,
    exact_asset_in: u64,
) -> Result<u64> {
    let reserve_balance_before = ctx.accounts.reserve_in_vault.amount;
    let asset_in_token_program = token_program_for_mint(
        &ctx.accounts.asset_in_mint,
        &ctx.accounts.token_program,
        &ctx.accounts.token_2022_program,
    )?;
    transfer_from_user_to_vault(
        ctx.accounts.trader.to_account_info(),
        ctx.accounts.trader_asset_in_account.to_account_info(),
        ctx.accounts.reserve_in_vault.to_account_info(),
        ctx.accounts.asset_in_mint.to_account_info(),
        asset_in_token_program,
        exact_asset_in,
        ctx.accounts.asset_in_mint.decimals,
    )?;
    ctx.accounts.reserve_in_vault.reload()?;
    ctx.accounts
        .reserve_in_vault
        .amount
        .checked_sub(reserve_balance_before)
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn move_swap_fee<'info>(ctx: &mut Context<SwapV2<'info>>, total_fee: u64) -> Result<u64> {
    if total_fee == 0 {
        return Ok(0);
    }
    let fee_balance_before = ctx.accounts.fee_in_vault.amount;
    let asset_in_token_program = token_program_for_mint(
        &ctx.accounts.asset_in_mint,
        &ctx.accounts.token_program,
        &ctx.accounts.token_2022_program,
    )?;
    transfer_from_vault_to_vault(
        ctx.accounts.market.to_account_info(),
        ctx.accounts.reserve_in_vault.to_account_info(),
        ctx.accounts.fee_in_vault.to_account_info(),
        ctx.accounts.asset_in_mint.to_account_info(),
        asset_in_token_program,
        total_fee,
        ctx.accounts.asset_in_mint.decimals,
        &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
    )?;
    ctx.accounts.reserve_in_vault.reload()?;
    ctx.accounts.fee_in_vault.reload()?;
    ctx.accounts
        .fee_in_vault
        .amount
        .checked_sub(fee_balance_before)
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn apply_swap_state(
    market_side_in: &mut MarketSideV2,
    market_side_out: &mut MarketSideV2,
    amount_in_after_fee: u64,
    amount_out: u64,
    fee_credit: u64,
    operator_fee_bps: u16,
) -> Result<()> {
    require_gte!(
        market_side_out.reserve_ledger.cash_reserve,
        amount_out,
        ErrorCode::InsufficientMarketClaimCoverageV2
    );
    let next_out_reserve = market_side_out
        .reserve_ledger
        .live_reserve
        .checked_sub(amount_out)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    require_market_reserve_floor(
        next_out_reserve,
        market_side_out.claim_ledger.protected_claim_supply,
        market_side_out.buffer_book.required_buffer,
    )?;

    market_side_in.reserve_ledger.live_reserve = market_side_in
        .reserve_ledger
        .live_reserve
        .checked_add(amount_in_after_fee)
        .ok_or(ErrorCode::ReserveOverflow)?;
    market_side_in.reserve_ledger.cash_reserve = market_side_in
        .reserve_ledger
        .cash_reserve
        .checked_add(amount_in_after_fee)
        .ok_or(ErrorCode::ReserveOverflow)?;
    market_side_out.reserve_ledger.live_reserve = next_out_reserve;
    market_side_out.reserve_ledger.cash_reserve = market_side_out
        .reserve_ledger
        .cash_reserve
        .checked_sub(amount_out)
        .ok_or(ErrorCode::CashReserveUnderflow)?;
    market_side_in.record_fee_credit(fee_credit, operator_fee_bps)?;
    market_side_in.assert_claim_coverage()?;
    market_side_out.assert_claim_coverage()?;
    market_side_in.fee_ledger.assert_backed()?;
    Ok(())
}
