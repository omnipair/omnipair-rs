use anchor_lang::prelude::*;
use crate::state::rate_model::RateModel;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;

#[derive(Accounts)]
pub struct CreateRateModel<'info> {
    #[account(
        init,
        payer = payer,
        space = get_size_with_discriminator::<RateModel>(),
    )]
    pub rate_model: Account<'info, RateModel>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

impl<'info> CreateRateModel<'info> {
    pub fn handle_create(ctx: Context<Self>) -> Result<()> {
        let rate_model = &mut ctx.accounts.rate_model;
        rate_model.exp_rate = SCALED_NATURAL_LOG_OF_TWO / SECONDS_PER_DAY;
        rate_model.target_util_start = TARGET_UTIL_START;
        rate_model.target_util_end = TARGET_UTIL_END;
        
        Ok(())
    }
} 