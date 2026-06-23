use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ReserveShares {
    pub ylp_supply: u64,
}

impl ReserveShares {
    pub fn shares_for_deposit(&self, reserve_before: u64, deposit_amount: u64) -> Result<u64> {
        if self.ylp_supply == 0 || reserve_before == 0 {
            return Ok(deposit_amount);
        }
        let shares = (deposit_amount as u128)
            .checked_mul(self.ylp_supply as u128)
            .and_then(|value| value.checked_div(reserve_before as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        u64::try_from(shares).map_err(|_| ErrorCode::MarketMathOverflow.into())
    }

    pub fn reserve_for_burn(&self, reserve_before: u64, share_amount: u64) -> Result<u64> {
        require!(share_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.ylp_supply,
            share_amount,
            ErrorCode::InsufficientBalance
        );
        let reserve_amount = (share_amount as u128)
            .checked_mul(reserve_before as u128)
            .and_then(|value| value.checked_div(self.ylp_supply as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        u64::try_from(reserve_amount).map_err(|_| ErrorCode::MarketMathOverflow.into())
    }

    pub fn mint(&mut self, share_amount: u64) -> Result<()> {
        require!(share_amount > 0, ErrorCode::AmountZero);
        self.ylp_supply = self
            .ylp_supply
            .checked_add(share_amount)
            .ok_or(ErrorCode::SupplyOverflow)?;
        Ok(())
    }

    pub fn burn(&mut self, share_amount: u64) -> Result<()> {
        require!(share_amount > 0, ErrorCode::AmountZero);
        self.ylp_supply = self
            .ylp_supply
            .checked_sub(share_amount)
            .ok_or(ErrorCode::SupplyUnderflow)?;
        Ok(())
    }
}
