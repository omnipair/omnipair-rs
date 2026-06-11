use anchor_lang::prelude::*;

use crate::{constants::*, v2::state::Market};

#[derive(Accounts)]
pub struct ViewMarketState<'info> {
    #[account(
        seeds = [
            MARKET_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,
}

impl ViewMarketState<'_> {
    pub fn handle_view(ctx: Context<Self>) -> Result<()> {
        let market = &ctx.accounts.market;
        msg!(
            "Market: market={}, asset0={}, asset1={}, reduce_only={}, buffer_ratio_bps={}",
            market.key(),
            market.asset0_mint,
            market.asset1_mint,
            market.reduce_only,
            market.config.buffer_ratio_bps
        );
        msg!(
            "Market side0: reserve={}, protected_claims={}, required_buffer={}, fee_liability={}",
            market.side0.reserve_ledger.live_reserve,
            market.side0.claim_ledger.protected_claim_supply,
            market.side0.buffer_book.required_buffer,
            market.side0.fee_ledger.fee_liability
        );
        msg!(
            "Market side1: reserve={}, protected_claims={}, required_buffer={}, fee_liability={}",
            market.side1.reserve_ledger.live_reserve,
            market.side1.claim_ledger.protected_claim_supply,
            market.side1.buffer_book.required_buffer,
            market.side1.fee_ledger.fee_liability
        );
        Ok(())
    }
}
