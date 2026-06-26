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
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        address = REDUCE_ONLY_EMERGENCY_AUTHORITY @ ErrorCode::InvalidReduceOnlyAuthority
    )]
    pub authority_signer: Signer<'info>,
}

impl<'info> SetMarketReduceOnly<'info> {
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }

    pub fn handle_set(ctx: Context<Self>, args: SetMarketReduceOnlyArgs) -> Result<()> {
        let market = &mut ctx.accounts.market;
        market.reduce_only = args.reduce_only;

        emit_cpi!(MarketUpdated {
            market: market.key(),
            reduce_only: market.reduce_only,
            target_hlp_leverage_bps: market.config.target_hlp_leverage_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            manager_fee_bps: market.config.manager_fee_bps,
            protocol_fee_bps: market.config.protocol_fee_bps,
            metadata: MarketEventMetadata::new(ctx.accounts.authority_signer.key(), market.key())?,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/instructions/market/set_reduce_only.rs");
}
