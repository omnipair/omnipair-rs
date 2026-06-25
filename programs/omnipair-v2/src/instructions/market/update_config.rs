use anchor_lang::prelude::*;

use crate::{
    constants::*,
    events::{
        MarketConfigUpdateScheduled, MarketEventMetadata, MarketHealthUpdated, MarketUpdated,
    },
    state::{Market, MarketConfig, MarketTimelockAction},
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

    /// Must be the market manager (checked in the handler).
    pub authority_signer: Signer<'info>,
}

impl<'info> UpdateMarketConfig<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateMarketConfigArgs) -> Result<()> {
        let signer = ctx.accounts.authority_signer.key();
        let current_slot = Clock::get()?.slot;
        let market = &mut ctx.accounts.market;
        match market.prepare_config_update(signer, args.config, current_slot)? {
            MarketTimelockAction::Scheduled { execute_after_slot } => {
                emit_cpi!(MarketConfigUpdateScheduled {
                    market: market.key(),
                    execute_after_slot,
                    target_hlp_leverage_bps: args.config.target_hlp_leverage_bps,
                    swap_fee_bps: args.config.swap_fee_bps,
                    manager_fee_bps: args.config.manager_fee_bps,
                    protocol_fee_bps: args.config.protocol_fee_bps,
                    metadata: MarketEventMetadata::new(signer, market.key())?,
                });
                return Ok(());
            }
            MarketTimelockAction::Ready => {}
        }
        apply_config_update(market, args.config)?;
        market.clear_pending_config_update();

        emit_cpi!(MarketUpdated {
            market: market.key(),
            reduce_only: market.reduce_only,
            target_hlp_leverage_bps: market.config.target_hlp_leverage_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            manager_fee_bps: market.config.manager_fee_bps,
            protocol_fee_bps: market.config.protocol_fee_bps,
            metadata: MarketEventMetadata::new(signer, market.key())?,
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
            metadata: MarketEventMetadata::new(signer, market.key())?,
        });

        Ok(())
    }
}

fn apply_config_update(market: &mut Market, config: MarketConfig) -> Result<()> {
    config.validate()?;
    let previous_config = market.config;
    let previous_risk = market.risk;
    let previous_health = market.health;
    let previous_last_update_slot = market.last_update_slot;

    market.config = config;
    let result = market
        .refresh_risk()
        .and_then(|_| market.assert_market_health());
    if result.is_err() {
        market.config = previous_config;
        market.risk = previous_risk;
        market.health = previous_health;
        market.last_update_slot = previous_last_update_slot;
    }
    result
}
