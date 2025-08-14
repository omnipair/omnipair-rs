use anchor_lang::prelude::*;
use crate::state::{futarchy_authority::FutarchyAuthority, pair_config::PairConfig};
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, PAIR_CONFIG_SEED_PREFIX, BPS_DENOMINATOR};
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitPairConfigArgs {
    pub futarchy_fee_bps: u16,
    pub founder_fee_bps: u16,
    pub nonce: u64,
}

#[derive(Accounts)]
#[instruction(args: InitPairConfigArgs)]
pub struct InitPairConfig<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init,
        payer = authority_signer,
        space = get_size_with_discriminator::<PairConfig>(),
        seeds = [PAIR_CONFIG_SEED_PREFIX, &args.nonce.to_le_bytes()],
        bump
    )]
    pub pair_config: Account<'info, PairConfig>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitPairConfig<'info> {
    pub fn handle_init(ctx: Context<Self>, args: InitPairConfigArgs) -> Result<()> {
        let pair_config = &mut ctx.accounts.pair_config;
        let futarchy_authority = &mut ctx.accounts.futarchy_authority;

        // Ensure args.nonce is exactly last + 1
        let expected_nonce = futarchy_authority.last_config_nonce.checked_add(1).ok_or(ErrorCode::Overflow)?;
        require_eq!(args.nonce, expected_nonce, ErrorCode::InvalidNonce);

        // validate BPS bounds
        require_gte!(BPS_DENOMINATOR, args.futarchy_fee_bps, ErrorCode::InvalidFutarchyFeeBps);
        require_gte!(BPS_DENOMINATOR, args.founder_fee_bps, ErrorCode::InvalidFounderFeeBps);

        pair_config.set_inner(PairConfig::initialize(
            args.futarchy_fee_bps,
            args.founder_fee_bps,
            args.nonce,
            ctx.bumps.pair_config,
        ));

        futarchy_authority.last_config_nonce = args.nonce;

        Ok(())
    }
}