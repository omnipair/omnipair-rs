use anchor_lang::prelude::*;
use omnipair::FlashLoanCallbackData;

pub mod constants;
pub mod errors;
pub mod instruction_math;
pub mod instructions;
pub mod state;
pub mod types;
pub mod utils;

pub use errors::LeverageError;
pub use state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX};
pub use types::InternalCallbackData;

// Re-export everything from each instruction module so Anchor's #[program] macro can
// find both the account structs and the generated __client_accounts_* types at the crate root.
pub use instructions::multiply::*;
pub use instructions::close_multiply::*;
pub use instructions::flash_loan_callback::*;

declare_id!("7S6gLNQXrx3GtR91xnF2ZTjdPeJfbMq79u4TovRDQEBn");

#[program]
pub mod omnipair_leverage {
    use super::*;

    pub fn multiply<'info>(
        ctx: Context<'_, '_, '_, 'info, Multiply<'info>>,
        is_lev_collateral0: bool,
        lev_collateral_amount: u64,
        multiplier_bps: u64,
        max_slippage_bps: u64,
    ) -> Result<()> {
        instructions::multiply::handle(ctx, is_lev_collateral0, lev_collateral_amount, multiplier_bps, max_slippage_bps)
    }

    pub fn close_multiply<'info>(
        ctx: Context<'_, '_, 'info, 'info, CloseMultiply<'info>>,
        is_lev_collateral0: bool,
        min_collateral_out: u64,
    ) -> Result<()> {
        instructions::close_multiply::handle(ctx, is_lev_collateral0, min_collateral_out)
    }

    pub fn flash_loan_callback<'info>(
        ctx: Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
        callback_data: FlashLoanCallbackData,
    ) -> Result<()> {
        instructions::flash_loan_callback::handle(ctx, callback_data)
    }
}
