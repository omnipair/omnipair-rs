use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketAuthorityUpdated, MarketEventMetadata},
    state::Market,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetOperatorArgs {
    pub new_operator: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetManagerArgs {
    pub new_manager: Pubkey,
}

/// Manager-only role management: the manager sets the market operator identity
/// and may rotate the manager role itself.
#[event_cpi]
#[derive(Accounts)]
pub struct SetMarketAuthority<'info> {
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

    /// The current market manager; only the manager may rotate roles.
    pub manager: Signer<'info>,
}

impl<'info> SetMarketAuthority<'info> {
    pub fn handle_set_operator(ctx: Context<Self>, args: SetOperatorArgs) -> Result<()> {
        require_keys_neq!(
            args.new_operator,
            Pubkey::default(),
            ErrorCode::InvalidArgument
        );
        let signer = ctx.accounts.manager.key();
        let market = &mut ctx.accounts.market;
        market.assert_manager(signer)?;
        market.operator = args.new_operator;
        emit_cpi!(MarketAuthorityUpdated {
            market: market.key(),
            manager: market.manager,
            operator: market.operator,
            metadata: MarketEventMetadata::new(signer, market.key())?,
        });
        Ok(())
    }

    pub fn handle_set_manager(ctx: Context<Self>, args: SetManagerArgs) -> Result<()> {
        require_keys_neq!(
            args.new_manager,
            Pubkey::default(),
            ErrorCode::InvalidArgument
        );
        let signer = ctx.accounts.manager.key();
        let market = &mut ctx.accounts.market;
        market.assert_manager(signer)?;
        market.manager = args.new_manager;
        emit_cpi!(MarketAuthorityUpdated {
            market: market.key(),
            manager: market.manager,
            operator: market.operator,
            metadata: MarketEventMetadata::new(signer, market.key())?,
        });
        Ok(())
    }
}
