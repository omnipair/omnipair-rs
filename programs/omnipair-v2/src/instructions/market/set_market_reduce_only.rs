use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketUpdated},
    state::Market,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetMarketReduceOnlyArgs {
    pub reduce_only: bool,
}

#[event_cpi]
#[derive(Accounts)]
pub struct SetMarketReduceOnly<'info> {
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
    pub market: Box<Account<'info, Market>>,

    #[account(address = market.operator @ ErrorCode::InvalidMarket)]
    pub operator: Signer<'info>,
}

impl<'info> SetMarketReduceOnly<'info> {
    pub fn handle_set(ctx: Context<Self>, args: SetMarketReduceOnlyArgs) -> Result<()> {
        let market = &mut ctx.accounts.market;
        market.reduce_only = args.reduce_only;

        emit_cpi!(MarketUpdated {
            market: market.key(),
            reduce_only: market.reduce_only,
            buffer_ratio_bps: market.config.buffer_ratio_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            operator_fee_bps: market.config.operator_fee_bps,
            metadata: MarketEventMetadata::new(ctx.accounts.operator.key(), market.key()),
        });

        Ok(())
    }
}
