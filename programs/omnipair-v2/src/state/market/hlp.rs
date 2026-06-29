use anchor_lang::prelude::*;

use super::{accrue_fee_liability, Debt, Market, MarketAsset, MarketSide};
use crate::{constants::NAD, errors::ErrorCode};

pub(crate) use crate::state::market::transitions::hedge::HlpRebalanceReceipt;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct HlpVault {
    pub target_side: u8,
    pub ylp_vault: Pubkey,
    pub ylp_shares: u64,
    pub debt_shares: u128,
    pub debt_principal: u128,
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
    pub fn initialize(&mut self, target_side: MarketAsset, ylp_vault: Pubkey, current_slot: u64) {
        self.target_side = target_side.code();
        self.ylp_vault = ylp_vault;
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

    pub fn credit_ylp(&mut self, shares: u64) -> Result<()> {
        self.ylp_shares = self
            .ylp_shares
            .checked_add(shares)
            .ok_or(ErrorCode::SupplyOverflow)?;
        Ok(())
    }

    pub fn debit_ylp(&mut self, shares: u64) -> Result<()> {
        self.ylp_shares = self
            .ylp_shares
            .checked_sub(shares)
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

    pub fn add_debt_principal(&mut self, amount: u64) -> Result<()> {
        self.debt_principal = self
            .debt_principal
            .checked_add(amount as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        Ok(())
    }

    pub fn realize_debt_repay(&mut self, repaid: u64, borrow_index_nad: u128) -> Result<u64> {
        let total_debt = Debt::shares_to_debt(self.debt_shares, borrow_index_nad)?;
        let principal = self.debt_principal.min(total_debt);
        let (principal_repaid, interest_paid) =
            crate::math::realized_interest_split(repaid, total_debt, principal)?;
        self.debt_principal = self.debt_principal.saturating_sub(principal_repaid as u128);
        Ok(interest_paid)
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
            self.ylp_shares,
            base_side.fees.swap_fee_growth_index_nad,
            self.base_swap_fee_checkpoint_nad,
        )?;
        let base_interest_amount = accrue_fee_liability(
            self.ylp_shares,
            base_side.fees.interest_growth_index_nad,
            self.base_interest_checkpoint_nad,
        )?;
        let quote_swap_fee_amount = accrue_fee_liability(
            self.ylp_shares,
            quote_side.fees.swap_fee_growth_index_nad,
            self.quote_swap_fee_checkpoint_nad,
        )?;
        let quote_interest_amount = accrue_fee_liability(
            self.ylp_shares,
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

impl Market {
    pub fn open_hedge(
        &mut self,
        target_asset: MarketAsset,
        deposit_amount: u64,
        min_hlp_amount: u64,
    ) -> Result<crate::state::market::transitions::hedge::HedgeReceipt> {
        crate::state::market::transitions::hedge::OpenHedge::new(
            target_asset,
            deposit_amount,
            min_hlp_amount,
        )
        .apply(self)
    }

    pub fn close_hedge(
        &mut self,
        target_asset: MarketAsset,
        hlp_amount: u64,
    ) -> Result<crate::state::market::transitions::hedge::HedgeReceipt> {
        crate::state::market::transitions::hedge::CloseHedge::new(target_asset, hlp_amount)
            .apply(self)
    }

    pub fn checkpoint_hlp_vaults(&mut self, current_slot: u64) -> Result<(i128, i128)> {
        crate::state::market::transitions::hedge::checkpoint_hlp_vaults(self, current_slot)
    }

    pub fn rebalance_hlp_vaults(
        &mut self,
        current_slot: u64,
    ) -> Result<(
        crate::state::market::transitions::hedge::HlpRebalanceReceipt,
        crate::state::market::transitions::hedge::HlpRebalanceReceipt,
    )> {
        crate::state::market::transitions::hedge::rebalance_hlp_vaults(self, current_slot)
    }

    pub fn checkpoint_hlp_yield_from_ylp(&mut self, target_asset: MarketAsset) -> Result<()> {
        crate::state::market::transitions::hedge::checkpoint_hlp_yield_from_ylp(self, target_asset)
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/state/hlp.rs");
}
