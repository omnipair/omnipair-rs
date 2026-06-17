use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketHealthUpdated, MarketUpdated},
    state::{Market, MarketConfig},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateMarketConfigArgs {
    pub config: MarketConfig,
}

#[event_cpi]
#[derive(Accounts)]
pub struct UpdateMarketConfig<'info> {
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

    #[account(address = market.operator @ ErrorCode::InvalidMarket)]
    pub operator: Signer<'info>,
}

impl<'info> UpdateMarketConfig<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateMarketConfigArgs) -> Result<()> {
        let market = &mut ctx.accounts.market;
        apply_config_update(market, args.config)?;

        emit_cpi!(MarketUpdated {
            market: market.key(),
            reduce_only: market.reduce_only,
            buffer_ratio_bps: market.config.buffer_ratio_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            operator_fee_bps: market.config.operator_fee_bps,
            protocol_fee_bps: market.config.protocol_fee_bps,
            metadata: MarketEventMetadata::new(ctx.accounts.operator.key(), market.key())?,
        });
        emit_cpi!(MarketHealthUpdated {
            market: market.key(),
            recognized_base_collateral_for_quote_debt: market
                .health
                .recognized_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: market
                .health
                .recognized_quote_collateral_for_base_debt,
            effective_base_debt_nad: market.health.effective_base_debt_nad,
            effective_quote_debt_nad: market.health.effective_quote_debt_nad,
            base_debt_health_bps: market.health.base_debt_health_bps,
            quote_debt_health_bps: market.health.quote_debt_health_bps,
            metadata: MarketEventMetadata::new(ctx.accounts.operator.key(), market.key())?,
        });

        Ok(())
    }
}

fn apply_config_update(market: &mut Market, config: MarketConfig) -> Result<()> {
    config.validate()?;
    let previous_config = market.config;
    let previous_base_side = market.base_side;
    let previous_quote_side = market.quote_side;
    let previous_risk_book = market.risk_book;
    let previous_health = market.health;
    let previous_last_update_slot = market.last_update_slot;

    market.apply_buffer_ratio_update(config.buffer_ratio_bps)?;
    market.config = config;
    let result = market
        .refresh_risk_book()
        .and_then(|_| market.assert_market_health());
    if result.is_err() {
        market.config = previous_config;
        market.base_side = previous_base_side;
        market.quote_side = previous_quote_side;
        market.risk_book = previous_risk_book;
        market.health = previous_health;
        market.last_update_slot = previous_last_update_slot;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::{BufferLedger, DebtBook, MarketSide, ReserveLedger},
    };

    fn market_side(asset_mint: Pubkey, live_reserve: u64) -> MarketSide {
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
                live_reserve,
                cash_reserve: live_reserve,
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
            swap_fee_bps: 30,
            operator_fee_bps: 1_000,
            protocol_fee_bps: 0,
            buffer_ratio_bps: 2_000,
            fee_routing_k_nad: NAD,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: BPS_DENOMINATOR,
            max_daily_withdraw_bps: BPS_DENOMINATOR,
            spot_ema_divergence_bps: BPS_DENOMINATOR,
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

    #[test]
    fn config_update_refreshes_risk_book_for_health_events() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint, 1_000_000),
            market_side(quote_mint, 2_000_000),
            market_config(),
            [31_u8; 32],
            42,
            250,
        )
        .unwrap();

        let mut config = market_config();
        config.operator_fee_bps = 500;
        apply_config_update(&mut market, config).unwrap();

        assert_eq!(market.config.operator_fee_bps, 500);
        assert_eq!(market.risk_book.base_price_ema_nad, 2 * NAD);
        assert_eq!(market.risk_book.quote_price_ema_nad, NAD / 2);
        assert_eq!(market.risk_book.cached_spot_base_price_nad, 2 * NAD);
        assert_eq!(market.risk_book.cached_spot_quote_price_nad, NAD / 2);
    }

    #[test]
    fn config_update_rejects_new_health_floor_that_breaks_existing_debt() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint, 1_000_000),
            market_side(quote_mint, 1_000_000),
            market_config(),
            [32_u8; 32],
            42,
            249,
        )
        .unwrap();
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000, market.debt_book.base_borrow_index_nad).unwrap();
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 500_000;
        market.refresh_market_health().unwrap();
        assert!(market.health.base_debt_health_bps >= market.config.market_health_min_bps as u64);

        let previous_health = market.health.base_debt_health_bps;
        let previous_min_health = market.config.market_health_min_bps;
        let mut config = market.config;
        config.market_health_min_bps = 50_000;
        config.recognized_collateral_cap_bps = 50_000;

        let err = apply_config_update(&mut market, config).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientMarketHealth));
        assert_eq!(market.config.market_health_min_bps, previous_min_health);
        assert_eq!(market.health.base_debt_health_bps, previous_health);
    }
}
