use anchor_lang::prelude::*;
use crate::state::{futarchy_authority::FutarchyAuthority, pair_config::PairConfig};
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, PAIR_CONFIG_SEED_PREFIX};
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdatePairConfigArgs {
    pub futarchy_fee_bps: Option<u16>,
    pub founder_fee_bps: Option<u16>,
    pub nonce: u64,
}

#[derive(Accounts)]
#[instruction(args: UpdatePairConfigArgs)]
pub struct UpdatePairConfig<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    #[account(
        mut,
        seeds = [PAIR_CONFIG_SEED_PREFIX, &args.nonce.to_le_bytes()],
        bump
    )]
    pub pair_config: Account<'info, PairConfig>,

    pub system_program: Program<'info, System>,
}

impl<'info> UpdatePairConfig<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdatePairConfigArgs) -> Result<()> {
        let pair_config = &mut ctx.accounts.pair_config;

        // TODO: do validation on the new args and nonce
        
        PairConfig::update_if_some(&mut pair_config.futarchy_fee_bps, args.futarchy_fee_bps);
        PairConfig::update_if_some(&mut pair_config.founder_fee_bps, args.founder_fee_bps);

        Ok(())
    }
}