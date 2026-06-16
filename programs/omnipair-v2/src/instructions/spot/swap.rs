use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, SwapExecuted},
    generate_market_seeds,
    shared::{
        gamm_math::CPCurve,
        math::ceil_div,
        token::{
            transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
        },
    },
    state::{Market, MarketAsset},
    transitions::swap::Swap as SwapTransition,
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
    validate_swap_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub asset_in: MarketAsset,
    pub exact_asset_in: u64,
    pub min_asset_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SwapArgs)]
pub struct Swap<'info> {
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

    #[account(mut)]
    pub trader: Signer<'info>,

    pub asset_in_mint: Box<InterfaceAccount<'info, Mint>>,

    pub asset_out_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_in_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub reserve_out_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub fee_in_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub trader_asset_in_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub trader_asset_out_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Swap<'info> {
    pub fn validate(&self, args: &SwapArgs) -> Result<()> {
        self.market.assert_live()?;
        require!(args.exact_asset_in > 0, ErrorCode::AmountZero);
        require_gte!(
            self.trader_asset_in_account.amount,
            args.exact_asset_in,
            ErrorCode::InsufficientBalance
        );
        validate_swap_accounts(
            &self.market,
            args.asset_in,
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

    pub fn handle_swap(mut ctx: Context<Self>, args: SwapArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let trader_key = ctx.accounts.trader.key();
        let asset_in_mint_key = ctx.accounts.asset_in_mint.key();
        let asset_out_mint_key = ctx.accounts.asset_out_mint.key();
        let operator_fee_bps = ctx.accounts.market.config.operator_fee_bps;
        let protocol_fee_bps = ctx.accounts.market.config.protocol_fee_bps;
        let fee_routing_k_nad = ctx.accounts.market.config.fee_routing_k_nad;

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
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(amount_in_after_fee > 0, ErrorCode::InsufficientOutputAmount);

        let amount_out = {
            let (market_side_in, market_side_out) = ctx.accounts.market.swap_sides(args.asset_in);
            CPCurve::calculate_amount_out(
                market_side_in.reserve_ledger.live_reserve,
                market_side_out.reserve_ledger.live_reserve,
                amount_in_after_fee,
            )?
        };

        let trader_asset_out_balance_before = ctx.accounts.trader_asset_out_account.amount;
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
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.trader_asset_out_account.reload()?;
        let asset_out_credit = token_account_credit(
            trader_asset_out_balance_before,
            &ctx.accounts.trader_asset_out_account,
        )?;
        require_gte!(
            asset_out_credit,
            args.min_asset_out,
            ErrorCode::SlippageExceeded
        );

        let swap_receipt = {
            let (market_side_in, market_side_out) =
                ctx.accounts.market.swap_sides_mut(args.asset_in);
            SwapTransition::new(
                amount_in_after_fee,
                amount_out,
                fee_credit,
                operator_fee_bps,
                protocol_fee_bps,
                fee_routing_k_nad,
            )
            .apply(market_side_in, market_side_out)?
        };
        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        emit_cpi!(SwapExecuted {
            market: market_key,
            trader: trader_key,
            asset_in_mint: asset_in_mint_key,
            asset_out_mint: asset_out_mint_key,
            reserve_credit,
            amount_in_after_fee: swap_receipt.amount_in_after_fee,
            amount_out: swap_receipt.amount_out,
            fee_credit: swap_receipt.fee_credit,
            metadata: MarketEventMetadata::new(trader_key, market_key)?,
        });

        Ok(())
    }
}

fn receive_swap_inventory<'info>(
    ctx: &mut Context<Swap<'info>>,
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
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn move_swap_fee<'info>(ctx: &mut Context<Swap<'info>>, total_fee: u64) -> Result<u64> {
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
        &[&generate_market_seeds!(ctx.accounts.market)[..]],
    )?;
    ctx.accounts.reserve_in_vault.reload()?;
    ctx.accounts.fee_in_vault.reload()?;
    ctx.accounts
        .fee_in_vault
        .amount
        .checked_sub(fee_balance_before)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}
