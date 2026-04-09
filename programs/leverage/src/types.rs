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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_callback_data_roundtrip() {
        let original = InternalCallbackData {
            is_close: true,
            is_lev_collateral0: false,
            swap_amount_in: 1,
            min_amount_out: 2,
            repay_amount: 3,
        };
        let mut buf = Vec::new();
        original.serialize(&mut buf).unwrap();
        let decoded = InternalCallbackData::try_from_slice(&buf).unwrap();
        assert_eq!(decoded.is_close, original.is_close);
        assert_eq!(decoded.is_lev_collateral0, original.is_lev_collateral0);
        assert_eq!(decoded.swap_amount_in, original.swap_amount_in);
        assert_eq!(decoded.min_amount_out, original.min_amount_out);
        assert_eq!(decoded.repay_amount, original.repay_amount);
    }
}
