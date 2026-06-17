use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketHealthUpdated, SwapExecuted},
    generate_market_seeds,
    math::calculate_raw_amount_out,
    shared::{
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

        ctx.accounts.market.refresh_risk_book()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

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
            calculate_raw_amount_out(
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
        emit_cpi!(MarketHealthUpdated {
            market: market_key,
            recognized_base_collateral_for_quote_debt: ctx
                .accounts
                .market
                .health
                .recognized_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: ctx
                .accounts
                .market
                .health
                .recognized_quote_collateral_for_base_debt,
            effective_base_debt_nad: ctx.accounts.market.health.effective_base_debt_nad,
            effective_quote_debt_nad: ctx.accounts.market.health.effective_quote_debt_nad,
            base_debt_health_bps: ctx.accounts.market.health.base_debt_health_bps,
            quote_debt_health_bps: ctx.accounts.market.health.quote_debt_health_bps,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::{BufferLedger, MarketConfig, MarketSide, ReserveLedger},
    };

    const TEST_RESERVE: u64 = 1_000_000;

    fn market_side(asset_mint: Pubkey) -> MarketSide {
        MarketSide {
            asset_mint,
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: ReserveLedger {
                live_reserve: TEST_RESERVE,
                cash_reserve: TEST_RESERVE,
                reserved_liability: 0,
            },
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn market_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 0,
            operator_fee_bps: 0,
            protocol_fee_bps: 0,
            buffer_ratio_bps: 2_000,
            fee_routing_k_nad: NAD,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: BPS_DENOMINATOR,
            max_daily_withdraw_bps: BPS_DENOMINATOR,
            spot_ema_divergence_bps: 1_000,
            k_ema_drawdown_bps: BPS_DENOMINATOR,
            recognized_collateral_cap_bps: 15_000,
            market_health_min_bps: 11_000,
            effective_debt_weight_min_bps: BPS_DENOMINATOR,
            effective_debt_gamma_nad: NAD,
            soft_borrow_enabled: false,
            hedged_lp_enabled: true,
            start_time: 0,
        }
    }

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint),
            market_side(quote_mint),
            market_config(),
            [23_u8; 32],
            42,
            252,
        )
        .unwrap()
    }

    #[test]
    fn pre_swap_risk_snapshot_blocks_bootstrap_from_post_swap_spot() {
        let mut market = test_market();
        market.refresh_risk_book().unwrap();
        assert_eq!(market.risk_book.base_price_ema_nad, NAD);
        assert_eq!(market.risk_book.quote_price_ema_nad, NAD);

        let amount_in_after_fee = 900_000;
        let amount_out = calculate_raw_amount_out(
            market.base_side.reserve_ledger.live_reserve,
            market.quote_side.reserve_ledger.live_reserve,
            amount_in_after_fee,
        )
        .unwrap();
        {
            let (market_side_in, market_side_out) = market.swap_sides_mut(MarketAsset::Base);
            SwapTransition::new(amount_in_after_fee, amount_out, 0, 0, 0, NAD)
                .apply(market_side_in, market_side_out)
                .unwrap();
        }
        market.refresh_risk_book().unwrap();

        let err = market.assert_risk_circuit_breakers().unwrap_err();
        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::MarketRiskCircuitBreaker)
        );
    }
}
