use anchor_lang::prelude::*;

use super::{MarketAsset, MarketSide};
use crate::{constants::NAD, errors::ErrorCode, utils::market_math::accrue_fee_liability};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct HlpVault {
    pub target_side: u8,
    pub base_ylp_vault: Pubkey,
    pub quote_ylp_vault: Pubkey,
    pub ylp_base_shares: u64,
    pub ylp_quote_shares: u64,
    pub debt_shares: u128,
    pub hlp_supply: u64,
    pub pending_rebalance: i128,
    pub base_swap_fee_growth_index_nad: u128,
    pub base_interest_growth_index_nad: u128,
    pub quote_swap_fee_growth_index_nad: u128,
    pub quote_interest_growth_index_nad: u128,
    pub base_swap_fee_checkpoint_nad: u128,
    pub base_interest_checkpoint_nad: u128,
    pub quote_swap_fee_checkpoint_nad: u128,
    pub quote_interest_checkpoint_nad: u128,
    pub unallocated_base_swap_fee_amount: u64,
    pub unallocated_base_interest_amount: u64,
    pub unallocated_quote_swap_fee_amount: u64,
    pub unallocated_quote_interest_amount: u64,
    pub last_nav_nad: u128,
    pub cached_settlement_price_nad: u128,
    pub last_rebalance_slot: u64,
}

impl HlpVault {
    pub fn initialize(
        &mut self,
        target_side: MarketAsset,
        base_ylp_vault: Pubkey,
        quote_ylp_vault: Pubkey,
        current_slot: u64,
    ) {
        self.target_side = target_side.code();
        self.base_ylp_vault = base_ylp_vault;
        self.quote_ylp_vault = quote_ylp_vault;
        self.last_rebalance_slot = current_slot;
    }

    pub fn target_asset(&self) -> Result<MarketAsset> {
        MarketAsset::try_from_code(self.target_side)
    }

    pub fn mint_hlp(&mut self, amount: u64) -> Result<()> {
        require!(amount > 0, ErrorCode::AmountZero);
        self.hlp_supply = self
            .hlp_supply
            .checked_add(amount)
            .ok_or(ErrorCode::SupplyOverflow)?;
        Ok(())
    }

    pub fn burn_hlp(&mut self, amount: u64) -> Result<()> {
        require!(amount > 0, ErrorCode::AmountZero);
        self.hlp_supply = self
            .hlp_supply
            .checked_sub(amount)
            .ok_or(ErrorCode::SupplyUnderflow)?;
        Ok(())
    }

    pub fn credit_ylp(&mut self, base_shares: u64, quote_shares: u64) -> Result<()> {
        self.ylp_base_shares = self
            .ylp_base_shares
            .checked_add(base_shares)
            .ok_or(ErrorCode::SupplyOverflow)?;
        self.ylp_quote_shares = self
            .ylp_quote_shares
            .checked_add(quote_shares)
            .ok_or(ErrorCode::SupplyOverflow)?;
        Ok(())
    }

    pub fn debit_ylp(&mut self, base_shares: u64, quote_shares: u64) -> Result<()> {
        self.ylp_base_shares = self
            .ylp_base_shares
            .checked_sub(base_shares)
            .ok_or(ErrorCode::SupplyUnderflow)?;
        self.ylp_quote_shares = self
            .ylp_quote_shares
            .checked_sub(quote_shares)
            .ok_or(ErrorCode::SupplyUnderflow)?;
        Ok(())
    }

    pub fn add_debt_shares(&mut self, shares: u128) -> Result<()> {
        self.debt_shares = self
            .debt_shares
            .checked_add(shares)
            .ok_or(ErrorCode::DebtShareMathOverflow)?;
        Ok(())
    }

    pub fn remove_debt_shares(&mut self, shares: u128) -> Result<()> {
        self.debt_shares = self
            .debt_shares
            .checked_sub(shares)
            .ok_or(ErrorCode::DebtShareMathOverflow)?;
        Ok(())
    }

    pub fn checkpoint_yield_from_ylp(
        &mut self,
        base_side: &MarketSide,
        quote_side: &MarketSide,
    ) -> Result<()> {
        let base_swap_fee_amount = accrue_fee_liability(
            self.ylp_base_shares,
            base_side.fees.swap_fee_growth_index_nad,
            self.base_swap_fee_checkpoint_nad,
        )?;
        let base_interest_amount = accrue_fee_liability(
            self.ylp_base_shares,
            base_side.fees.interest_growth_index_nad,
            self.base_interest_checkpoint_nad,
        )?;
        let quote_swap_fee_amount = accrue_fee_liability(
            self.ylp_quote_shares,
            quote_side.fees.swap_fee_growth_index_nad,
            self.quote_swap_fee_checkpoint_nad,
        )?;
        let quote_interest_amount = accrue_fee_liability(
            self.ylp_quote_shares,
            quote_side.fees.interest_growth_index_nad,
            self.quote_interest_checkpoint_nad,
        )?;

        credit_hlp_growth(
            self.hlp_supply,
            &mut self.unallocated_base_swap_fee_amount,
            &mut self.base_swap_fee_growth_index_nad,
            base_swap_fee_amount,
        )?;
        credit_hlp_growth(
            self.hlp_supply,
            &mut self.unallocated_base_interest_amount,
            &mut self.base_interest_growth_index_nad,
            base_interest_amount,
        )?;
        credit_hlp_growth(
            self.hlp_supply,
            &mut self.unallocated_quote_swap_fee_amount,
            &mut self.quote_swap_fee_growth_index_nad,
            quote_swap_fee_amount,
        )?;
        credit_hlp_growth(
            self.hlp_supply,
            &mut self.unallocated_quote_interest_amount,
            &mut self.quote_interest_growth_index_nad,
            quote_interest_amount,
        )?;

        self.base_swap_fee_checkpoint_nad = base_side.fees.swap_fee_growth_index_nad;
        self.base_interest_checkpoint_nad = base_side.fees.interest_growth_index_nad;
        self.quote_swap_fee_checkpoint_nad = quote_side.fees.swap_fee_growth_index_nad;
        self.quote_interest_checkpoint_nad = quote_side.fees.interest_growth_index_nad;
        Ok(())
    }

    pub fn yield_growth_indexes(&self, market_asset: MarketAsset) -> (u128, u128) {
        match market_asset {
            MarketAsset::Base => (
                self.base_swap_fee_growth_index_nad,
                self.base_interest_growth_index_nad,
            ),
            MarketAsset::Quote => (
                self.quote_swap_fee_growth_index_nad,
                self.quote_interest_growth_index_nad,
            ),
        }
    }
}

fn credit_hlp_growth(
    hlp_supply: u64,
    unallocated_amount: &mut u64,
    growth_index_nad: &mut u128,
    new_amount: u64,
) -> Result<()> {
    *unallocated_amount = unallocated_amount
        .checked_add(new_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if hlp_supply == 0 || *unallocated_amount == 0 {
        return Ok(());
    }
    let growth_delta = (*unallocated_amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(hlp_supply as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    if growth_delta == 0 {
        return Ok(());
    }
    let allocated = growth_delta
        .checked_mul(hlp_supply as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated = u64::try_from(allocated).map_err(|_| ErrorCode::MarketMathOverflow)?;
    *growth_index_nad = growth_index_nad
        .checked_add(growth_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    *unallocated_amount = unallocated_amount
        .checked_sub(allocated)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::NAD;

    #[test]
    fn hlp_vault_checkpoints_owned_ylp_revenue_into_hlp_indexes() {
        let mut vault = HlpVault {
            ylp_base_shares: 50,
            hlp_supply: 25,
            ..HlpVault::default()
        };
        let mut base_side = MarketSide::default();
        let quote_side = MarketSide::default();
        base_side.fees.swap_fee_growth_index_nad = 2 * NAD as u128;
        base_side.fees.interest_growth_index_nad = 3 * NAD as u128;

        vault
            .checkpoint_yield_from_ylp(&base_side, &quote_side)
            .unwrap();

        assert_eq!(vault.base_swap_fee_growth_index_nad, 4 * NAD as u128);
        assert_eq!(vault.base_interest_growth_index_nad, 6 * NAD as u128);
        assert_eq!(
            vault.base_swap_fee_checkpoint_nad,
            base_side.fees.swap_fee_growth_index_nad
        );
        assert_eq!(
            vault.base_interest_checkpoint_nad,
            base_side.fees.interest_growth_index_nad
        );
    }
}
