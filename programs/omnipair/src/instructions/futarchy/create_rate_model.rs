use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::state::rate_model::RateModel;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateRateModelArgs {
    pub target_util_start_bps: u64,
    pub target_util_end_bps: u64,
    pub half_life_ms: u64,
    pub min_rate_bps: u64,
    pub max_rate_bps: u64,
    pub initial_rate_bps: u64,
}

#[derive(Accounts)]
pub struct CreateRateModel<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init,
        payer = authority_signer,
        space = get_size_with_discriminator::<RateModel>(),
    )]
    pub rate_model: Account<'info, RateModel>,

    pub system_program: Program<'info, System>,
}

impl<'info> CreateRateModel<'info> {
    pub fn validate(args: &CreateRateModelArgs) -> Result<()> {
        require!(
            RateModel::validate_util_bounds(args.target_util_start_bps, args.target_util_end_bps),
            ErrorCode::InvalidUtilBounds
        );
        require!(
            RateModel::validate_rate_params(
                args.half_life_ms,
                args.min_rate_bps,
                args.max_rate_bps,
                args.initial_rate_bps
            ),
            ErrorCode::InvalidRateParams
        );
        Ok(())
    }

    pub fn handle_create_rate_model(ctx: Context<Self>, args: CreateRateModelArgs) -> Result<()> {
        ctx.accounts.rate_model.set_inner(RateModel::new(
            args.target_util_start_bps,
            args.target_util_end_bps,
            args.half_life_ms,
            args.min_rate_bps,
            args.max_rate_bps,
            args.initial_rate_bps,
        ));

        msg!(
            "Rate model created: {} (util {}-{} bps, half_life {} ms, rates {}-{} bps, initial {} bps)",
            ctx.accounts.rate_model.key(),
            args.target_util_start_bps,
            args.target_util_end_bps,
            args.half_life_ms,
            args.min_rate_bps,
            args.max_rate_bps,
            args.initial_rate_bps
        );

        Ok(())
    }
}
