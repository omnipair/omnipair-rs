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
        args.config.validate()?;
        let market = &mut ctx.accounts.market;
        market.apply_buffer_ratio_update(args.config.buffer_ratio_bps)?;
        market.config = args.config;
        market.recompute_market_health_from_risk_book()?;

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
