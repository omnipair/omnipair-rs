use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, YieldRecipientUpdated},
    state::{Market, YieldAccount, YieldTokenKind},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetYieldRecipientArgs {
    pub token_kind: YieldTokenKind,
    pub recipient: Pubkey,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SetYieldRecipientArgs)]
pub struct SetYieldRecipient<'info> {
    #[account(
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
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            YIELD_ACCOUNT_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
            &[args.token_kind.code()],
        ],
        bump = yield_account.bump
    )]
    pub yield_account: Box<Account<'info, YieldAccount>>,
}

impl<'info> SetYieldRecipient<'info> {
    pub fn validate(&self, args: &SetYieldRecipientArgs) -> Result<()> {
        require_keys_neq!(
            args.recipient,
            Pubkey::default(),
            ErrorCode::InvalidRecipient
        );
        self.yield_account.assert_account(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
            args.token_kind,
        )
    }

    pub fn handle_set(ctx: Context<Self>, args: SetYieldRecipientArgs) -> Result<()> {
        ctx.accounts.yield_account.recipient = args.recipient;
        emit_cpi!(YieldRecipientUpdated {
            market: ctx.accounts.market.key(),
            owner: ctx.accounts.owner.key(),
            asset_mint: ctx.accounts.asset_mint.key(),
            token_kind: args.token_kind.code(),
            recipient: args.recipient,
            metadata: MarketEventMetadata::new(
                ctx.accounts.owner.key(),
                ctx.accounts.market.key()
            )?,
        });
        Ok(())
    }
}
