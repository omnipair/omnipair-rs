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

    pub authority: Signer<'info>,
}

impl<'info> SetMarketReduceOnly<'info> {
    pub fn validate(&self) -> Result<()> {
        require_reduce_only_authority(self.authority.key(), self.market.operator)
    }

    pub fn handle_set(ctx: Context<Self>, args: SetMarketReduceOnlyArgs) -> Result<()> {
        let market = &mut ctx.accounts.market;
        market.reduce_only = args.reduce_only;

        emit_cpi!(MarketUpdated {
            market: market.key(),
            reduce_only: market.reduce_only,
            buffer_ratio_bps: market.config.buffer_ratio_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            operator_fee_bps: market.config.operator_fee_bps,
            protocol_fee_bps: market.config.protocol_fee_bps,
            metadata: MarketEventMetadata::new(ctx.accounts.authority.key(), market.key())?,
        });

        Ok(())
    }
}

fn require_reduce_only_authority(authority: Pubkey, operator: Pubkey) -> Result<()> {
    require!(
        authority == operator || authority == REDUCE_ONLY_EMERGENCY_AUTHORITY,
        ErrorCode::InvalidReduceOnlyAuthority
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_only_authority_accepts_operator_or_emergency_authority() {
        let operator = Pubkey::new_unique();

        require_reduce_only_authority(operator, operator).unwrap();
        require_reduce_only_authority(REDUCE_ONLY_EMERGENCY_AUTHORITY, operator).unwrap();
    }

    #[test]
    fn reduce_only_authority_rejects_unrelated_signer() {
        let err =
            require_reduce_only_authority(Pubkey::new_unique(), Pubkey::new_unique()).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidReduceOnlyAuthority));
    }
}
