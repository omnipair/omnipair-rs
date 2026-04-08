use anchor_lang::prelude::*;

/// Params encoded into the flashloan `data` bytes and decoded in the callback.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InternalCallbackData {
    /// true = close (deleverage), false = open (multiply)
    pub is_close: bool,
    pub is_lev_collateral0: bool,
    /// open: total swap amount in (lev_collateral * multiplier). close: unused (0).
    pub swap_amount_in: u64,
    /// open: min position token out. close: min lev_collateral out after swap-back.
    pub min_amount_out: u64,
    /// open: borrow_amount + flashloan fee. close: debt_amount + flashloan fee.
    pub repay_amount: u64,
}
