use anchor_lang::prelude::*;

use crate::{
    constants::NAD,
    errors::ErrorCode,
    math::{
        calculate_normalized_amount_in, calculate_raw_amount_out, denormalize_from_nad_floor,
        market_spot_price_nad, normalize_to_nad,
    },
    shared::math::ceil_div,
    state::{Debt, HlpVault, Market, MarketAsset},
    transitions::{
        fee::{carry_forward_interest, carry_forward_swap_fees},
        reserve::{credit_reserve, debit_reserve},
    },
};

pub struct OpenHedge {
    pub target_asset: MarketAsset,
    pub deposit_amount: u64,
    pub min_hlp_amount: u64,
}

pub struct CloseHedge {
    pub target_asset: MarketAsset,
    pub hlp_amount: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HedgeReceipt {
    pub deposit_amount: u64,
    pub borrowed_amount: u64,
    pub base_ylp_amount: u64,
    pub quote_ylp_amount: u64,
    pub hlp_amount: u64,
    pub hlp_supply: u64,
    pub target_amount_out: u64,
    pub debt_repaid: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HlpRebalanceReceipt {
    pub target_asset: MarketAsset,
    pub ideal_delta: i128,
    pub executed_delta: i128,
    pub pending_rebalance: i128,
    pub base_ylp_mint_amount: u64,
    pub quote_ylp_mint_amount: u64,
    pub base_ylp_burn_amount: u64,
    pub quote_ylp_burn_amount: u64,
    pub debt_delta: i128,
    pub nav_nad: u128,
}

impl Default for HlpRebalanceReceipt {
    fn default() -> Self {
        Self {
            target_asset: MarketAsset::Base,
            ideal_delta: 0,
            executed_delta: 0,
            pending_rebalance: 0,
            base_ylp_mint_amount: 0,
            quote_ylp_mint_amount: 0,
            base_ylp_burn_amount: 0,
            quote_ylp_burn_amount: 0,
            debt_delta: 0,
            nav_nad: 0,
        }
    }
}

impl OpenHedge {
    pub fn new(target_asset: MarketAsset, deposit_amount: u64, min_hlp_amount: u64) -> Self {
        Self {
            target_asset,
            deposit_amount,
            min_hlp_amount,
        }
    }

    pub fn apply(self, market: &mut Market) -> Result<HedgeReceipt> {
        require!(self.deposit_amount > 0, ErrorCode::AmountZero);
        require!(
            market.config.hedged_lp_enabled,
            ErrorCode::InvalidMarketConfig
        );
        require_hlp_settlement_available(market, self.target_asset)?;
        let borrowed_amount =
            market.spot_value_in_opposite(self.target_asset, self.deposit_amount)?;
        require!(borrowed_amount > 0, ErrorCode::InsufficientLiquidity);
        checkpoint_hlp_yield_from_ylp(market, self.target_asset)?;

        let (base_ylp_amount, quote_ylp_amount, hlp_amount, hlp_supply) = match self.target_asset {
            MarketAsset::Base => open_base_hlp(market, self.deposit_amount, borrowed_amount)?,
            MarketAsset::Quote => open_quote_hlp(market, self.deposit_amount, borrowed_amount)?,
        };
        require_gte!(hlp_amount, self.min_hlp_amount, ErrorCode::SlippageExceeded);
        market.refresh_market_health()?;
        market.assert_market_health()?;
        Ok(HedgeReceipt {
            deposit_amount: self.deposit_amount,
            borrowed_amount,
            base_ylp_amount,
            quote_ylp_amount,
            hlp_amount,
            hlp_supply,
            target_amount_out: 0,
            debt_repaid: 0,
        })
    }
}

impl CloseHedge {
    pub fn new(target_asset: MarketAsset, hlp_amount: u64) -> Self {
        Self {
            target_asset,
            hlp_amount,
        }
    }

    pub fn apply(self, market: &mut Market) -> Result<HedgeReceipt> {
        require!(self.hlp_amount > 0, ErrorCode::AmountZero);
        require_hlp_settlement_available(market, self.target_asset)?;
        checkpoint_hlp_yield_from_ylp(market, self.target_asset)?;
        let receipt = match self.target_asset {
            MarketAsset::Base => close_base_hlp(market, self.hlp_amount)?,
            MarketAsset::Quote => close_quote_hlp(market, self.hlp_amount)?,
        };
        market.refresh_market_health()?;
        Ok(receipt)
    }
}

pub fn checkpoint_hlp_vaults(market: &mut Market, current_slot: u64) -> Result<(i128, i128)> {
    let base_delta = checkpoint_one_hlp(market, MarketAsset::Base, current_slot)?;
    let quote_delta = checkpoint_one_hlp(market, MarketAsset::Quote, current_slot)?;
    Ok((base_delta, quote_delta))
}

pub fn rebalance_hlp_vaults(
    market: &mut Market,
    current_slot: u64,
) -> Result<(HlpRebalanceReceipt, HlpRebalanceReceipt)> {
    let base_receipt = rebalance_one_hlp(market, MarketAsset::Base, current_slot)?;
    let quote_receipt = rebalance_one_hlp(market, MarketAsset::Quote, current_slot)?;
    Ok((base_receipt, quote_receipt))
}

fn open_base_hlp(
    market: &mut Market,
    base_deposit: u64,
    quote_borrow: u64,
) -> Result<(u64, u64, u64, u64)> {
    require_hlp_borrow_headroom(&market.quote_side, quote_borrow)?;
    let hlp_supply_before = market.base_hlp_vault.hlp_supply;
    let nav_before_nad = if hlp_supply_before == 0 {
        0
    } else {
        hlp_nav_nad(market, MarketAsset::Base)?
    };
    let base_reserve_before = market.base_side.reserves.live_reserve;
    let quote_reserve_before = market.quote_side.reserves.live_reserve;
    let base_ylp = market
        .base_side
        .shares
        .shares_for_deposit(base_reserve_before, base_deposit)?;
    let quote_ylp = market
        .quote_side
        .shares
        .shares_for_deposit(quote_reserve_before, quote_borrow)?;
    credit_reserve(&mut market.base_side, base_deposit, true)?;
    credit_reserve(&mut market.quote_side, quote_borrow, false)?;
    market.base_side.shares.mint(base_ylp)?;
    market.quote_side.shares.mint(quote_ylp)?;
    let debt_shares = Debt::debt_to_shares(quote_borrow, market.debt.quote_borrow_index_nad)?;
    market.base_hlp_vault.add_debt_shares(debt_shares)?;
    market.base_hlp_vault.add_debt_principal(quote_borrow)?;
    market.base_hlp_vault.credit_ylp(base_ylp, quote_ylp)?;
    let current_nav_nad = hlp_nav_nad(market, MarketAsset::Base)?;
    let hlp_amount = if hlp_supply_before == 0 {
        base_deposit
    } else {
        let delta_nav_nad = current_nav_nad
            .checked_sub(nav_before_nad)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        hlp_shares_for_delta_nav(
            delta_nav_nad,
            nav_before_nad.max(market.base_hlp_vault.last_nav_nad),
            hlp_supply_before,
        )?
    };
    market.base_hlp_vault.mint_hlp(hlp_amount)?;
    market.base_hlp_vault.last_nav_nad = current_nav_nad;
    market.base_hlp_vault.cached_settlement_price_nad =
        current_settlement_price_nad(market, MarketAsset::Base)?;
    Ok((
        base_ylp,
        quote_ylp,
        hlp_amount,
        market.base_hlp_vault.hlp_supply,
    ))
}

fn open_quote_hlp(
    market: &mut Market,
    quote_deposit: u64,
    base_borrow: u64,
) -> Result<(u64, u64, u64, u64)> {
    require_hlp_borrow_headroom(&market.base_side, base_borrow)?;
    let hlp_supply_before = market.quote_hlp_vault.hlp_supply;
    let nav_before_nad = if hlp_supply_before == 0 {
        0
    } else {
        hlp_nav_nad(market, MarketAsset::Quote)?
    };
    let base_reserve_before = market.base_side.reserves.live_reserve;
    let quote_reserve_before = market.quote_side.reserves.live_reserve;
    let base_ylp = market
        .base_side
        .shares
        .shares_for_deposit(base_reserve_before, base_borrow)?;
    let quote_ylp = market
        .quote_side
        .shares
        .shares_for_deposit(quote_reserve_before, quote_deposit)?;
    credit_reserve(&mut market.base_side, base_borrow, false)?;
    credit_reserve(&mut market.quote_side, quote_deposit, true)?;
    market.base_side.shares.mint(base_ylp)?;
    market.quote_side.shares.mint(quote_ylp)?;
    let debt_shares = Debt::debt_to_shares(base_borrow, market.debt.base_borrow_index_nad)?;
    market.quote_hlp_vault.add_debt_shares(debt_shares)?;
    market.quote_hlp_vault.add_debt_principal(base_borrow)?;
    market.quote_hlp_vault.credit_ylp(base_ylp, quote_ylp)?;
    let current_nav_nad = hlp_nav_nad(market, MarketAsset::Quote)?;
    let hlp_amount = if hlp_supply_before == 0 {
        quote_deposit
    } else {
        let delta_nav_nad = current_nav_nad
            .checked_sub(nav_before_nad)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        hlp_shares_for_delta_nav(
            delta_nav_nad,
            nav_before_nad.max(market.quote_hlp_vault.last_nav_nad),
            hlp_supply_before,
        )?
    };
    market.quote_hlp_vault.mint_hlp(hlp_amount)?;
    market.quote_hlp_vault.last_nav_nad = current_nav_nad;
    market.quote_hlp_vault.cached_settlement_price_nad =
        current_settlement_price_nad(market, MarketAsset::Quote)?;
    Ok((
        base_ylp,
        quote_ylp,
        hlp_amount,
        market.quote_hlp_vault.hlp_supply,
    ))
}

fn close_base_hlp(market: &mut Market, hlp_amount: u64) -> Result<HedgeReceipt> {
    let supply = market.base_hlp_vault.hlp_supply;
    require_gte!(supply, hlp_amount, ErrorCode::InsufficientBalance);
    let base_ylp = proportional(market.base_hlp_vault.ylp_base_shares, hlp_amount, supply)?;
    let quote_ylp = proportional(market.base_hlp_vault.ylp_quote_shares, hlp_amount, supply)?;
    let quote_debt_shares =
        proportional_u128(market.base_hlp_vault.debt_shares, hlp_amount, supply)?;
    let base_out = market
        .base_side
        .shares
        .reserve_for_burn(market.base_side.reserves.live_reserve, base_ylp)?;
    let quote_redeemed = market
        .quote_side
        .shares
        .reserve_for_burn(market.quote_side.reserves.live_reserve, quote_ylp)?;
    let debt_repaid = Debt::shares_to_debt(quote_debt_shares, market.debt.quote_borrow_index_nad)?;
    let debt_repaid = u64::try_from(debt_repaid).map_err(|_| ErrorCode::DebtMathOverflow)?;
    let base_out = settled_close_target_amount(
        &market.base_side,
        &market.quote_side,
        base_out,
        quote_redeemed,
        debt_repaid,
    )?;
    debit_reserve(&mut market.base_side, base_out, true)?;
    debit_reserve(&mut market.quote_side, debt_repaid, false)?;
    market.base_side.shares.burn(base_ylp)?;
    market.quote_side.shares.burn(quote_ylp)?;
    market.base_side.assert_share_backing()?;
    market.quote_side.assert_share_backing()?;
    market.base_hlp_vault.debit_ylp(base_ylp, quote_ylp)?;
    let _interest_paid = market
        .base_hlp_vault
        .realize_debt_repay(debt_repaid, market.debt.quote_borrow_index_nad)?;
    market
        .base_hlp_vault
        .remove_debt_shares(quote_debt_shares)?;
    market.base_hlp_vault.burn_hlp(hlp_amount)?;
    market.base_hlp_vault.last_nav_nad = hlp_nav_nad(market, MarketAsset::Base)?;
    market.base_hlp_vault.cached_settlement_price_nad =
        current_settlement_price_nad(market, MarketAsset::Base)?;
    Ok(HedgeReceipt {
        hlp_amount,
        base_ylp_amount: base_ylp,
        quote_ylp_amount: quote_ylp,
        hlp_supply: market.base_hlp_vault.hlp_supply,
        target_amount_out: base_out,
        debt_repaid,
        ..HedgeReceipt::default()
    })
}

fn close_quote_hlp(market: &mut Market, hlp_amount: u64) -> Result<HedgeReceipt> {
    let supply = market.quote_hlp_vault.hlp_supply;
    require_gte!(supply, hlp_amount, ErrorCode::InsufficientBalance);
    let base_ylp = proportional(market.quote_hlp_vault.ylp_base_shares, hlp_amount, supply)?;
    let quote_ylp = proportional(market.quote_hlp_vault.ylp_quote_shares, hlp_amount, supply)?;
    let base_debt_shares =
        proportional_u128(market.quote_hlp_vault.debt_shares, hlp_amount, supply)?;
    let quote_out = market
        .quote_side
        .shares
        .reserve_for_burn(market.quote_side.reserves.live_reserve, quote_ylp)?;
    let base_redeemed = market
        .base_side
        .shares
        .reserve_for_burn(market.base_side.reserves.live_reserve, base_ylp)?;
    let debt_repaid = Debt::shares_to_debt(base_debt_shares, market.debt.base_borrow_index_nad)?;
    let debt_repaid = u64::try_from(debt_repaid).map_err(|_| ErrorCode::DebtMathOverflow)?;
    let quote_out = settled_close_target_amount(
        &market.quote_side,
        &market.base_side,
        quote_out,
        base_redeemed,
        debt_repaid,
    )?;
    debit_reserve(&mut market.quote_side, quote_out, true)?;
    debit_reserve(&mut market.base_side, debt_repaid, false)?;
    market.base_side.shares.burn(base_ylp)?;
    market.quote_side.shares.burn(quote_ylp)?;
    market.base_side.assert_share_backing()?;
    market.quote_side.assert_share_backing()?;
    market.quote_hlp_vault.debit_ylp(base_ylp, quote_ylp)?;
    let _interest_paid = market
        .quote_hlp_vault
        .realize_debt_repay(debt_repaid, market.debt.base_borrow_index_nad)?;
    market
        .quote_hlp_vault
        .remove_debt_shares(base_debt_shares)?;
    market.quote_hlp_vault.burn_hlp(hlp_amount)?;
    market.quote_hlp_vault.last_nav_nad = hlp_nav_nad(market, MarketAsset::Quote)?;
    market.quote_hlp_vault.cached_settlement_price_nad =
        current_settlement_price_nad(market, MarketAsset::Quote)?;
    Ok(HedgeReceipt {
        hlp_amount,
        base_ylp_amount: base_ylp,
        quote_ylp_amount: quote_ylp,
        hlp_supply: market.quote_hlp_vault.hlp_supply,
        target_amount_out: quote_out,
        debt_repaid,
        ..HedgeReceipt::default()
    })
}

fn settled_close_target_amount(
    target_side: &crate::state::MarketSide,
    borrowed_side: &crate::state::MarketSide,
    target_redeemed: u64,
    borrowed_redeemed: u64,
    debt_repaid: u64,
) -> Result<u64> {
    let target_reserve_after_burn = target_side
        .reserves
        .live_reserve
        .checked_sub(target_redeemed)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    let borrowed_reserve_after_burn = borrowed_side
        .reserves
        .live_reserve
        .checked_sub(borrowed_redeemed)
        .ok_or(ErrorCode::ReserveUnderflow)?;

    if borrowed_redeemed == debt_repaid {
        return Ok(target_redeemed);
    }

    if borrowed_redeemed > debt_repaid {
        let surplus_borrowed = borrowed_redeemed
            .checked_sub(debt_repaid)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let target_from_surplus = calculate_raw_amount_out(
            borrowed_reserve_after_burn,
            target_reserve_after_burn,
            surplus_borrowed,
        )?;
        return target_redeemed
            .checked_add(target_from_surplus)
            .ok_or(ErrorCode::MarketMathOverflow.into());
    }

    let borrowed_shortfall = debt_repaid
        .checked_sub(borrowed_redeemed)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let target_needed = calculate_normalized_amount_in(
        target_reserve_after_burn as u128,
        borrowed_reserve_after_burn as u128,
        borrowed_shortfall as u128,
    )?;
    let target_needed = u64::try_from(target_needed).map_err(|_| ErrorCode::MarketMathOverflow)?;
    require_gte!(
        target_redeemed,
        target_needed,
        ErrorCode::HlpSettlementUnavailable
    );
    target_redeemed
        .checked_sub(target_needed)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn rebalance_one_hlp(
    market: &mut Market,
    target_asset: MarketAsset,
    current_slot: u64,
) -> Result<HlpRebalanceReceipt> {
    checkpoint_hlp_yield_from_ylp(market, target_asset)?;
    let ideal_delta = current_hlp_ideal_delta(market, target_asset)?;
    let receipt = if ideal_delta > 0 {
        leverage_up_balanced(market, target_asset, ideal_delta)?
    } else if ideal_delta < 0 {
        deleverage_balanced(market, target_asset, ideal_delta)?
    } else {
        HlpRebalanceReceipt {
            target_asset,
            ..HlpRebalanceReceipt::default()
        }
    };
    refresh_hlp_after_rebalance(market, target_asset, current_slot, receipt)
}

fn current_hlp_ideal_delta(market: &Market, target_asset: MarketAsset) -> Result<i128> {
    let (collateral, debt) = match target_asset {
        MarketAsset::Base => (
            hlp_collateral_value_nad(market, MarketAsset::Base, &market.base_hlp_vault)?,
            hlp_debt_value_nad(market, MarketAsset::Base)?,
        ),
        MarketAsset::Quote => (
            hlp_collateral_value_nad(market, MarketAsset::Quote, &market.quote_hlp_vault)?,
            hlp_debt_value_nad(market, MarketAsset::Quote)?,
        ),
    };
    (collateral as i128)
        .checked_sub(debt.checked_mul(2).ok_or(ErrorCode::DebtMathOverflow)? as i128)
        .ok_or(ErrorCode::DebtMathOverflow.into())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BalancedRebalanceAmounts {
    target_leg_amount: u64,
    borrowed_leg_amount: u64,
    debt_amount: u64,
}

fn leverage_up_balanced(
    market: &mut Market,
    target_asset: MarketAsset,
    ideal_delta: i128,
) -> Result<HlpRebalanceReceipt> {
    let target_total_amount =
        feasible_leverage_up_target_amount(market, target_asset, ideal_delta as u128)?;
    let amounts =
        balanced_rebalance_amounts_from_target_amount(market, target_asset, target_total_amount)?;
    if amounts.target_leg_amount == 0
        || amounts.borrowed_leg_amount == 0
        || amounts.debt_amount == 0
    {
        return Ok(HlpRebalanceReceipt {
            target_asset,
            ideal_delta,
            ..HlpRebalanceReceipt::default()
        });
    }
    let borrowed_asset = target_asset.opposite();
    require_hlp_borrow_headroom(market.side(borrowed_asset)?, amounts.debt_amount)?;

    let target_side = market.side_mut(target_asset)?;
    let target_reserve_before = target_side.reserves.live_reserve;
    let target_ylp_amount = target_side
        .shares
        .shares_for_deposit(target_reserve_before, amounts.target_leg_amount)?;
    credit_reserve(target_side, amounts.target_leg_amount, false)?;
    target_side.shares.mint(target_ylp_amount)?;
    target_side.assert_share_backing()?;

    let borrowed_side = market.side_mut(borrowed_asset)?;
    let reserve_before = borrowed_side.reserves.live_reserve;
    let ylp_amount = borrowed_side
        .shares
        .shares_for_deposit(reserve_before, amounts.borrowed_leg_amount)?;
    credit_reserve(borrowed_side, amounts.borrowed_leg_amount, false)?;
    borrowed_side.shares.mint(ylp_amount)?;
    borrowed_side.assert_share_backing()?;

    let debt_shares = match target_asset {
        MarketAsset::Base => {
            Debt::debt_to_shares(amounts.debt_amount, market.debt.quote_borrow_index_nad)?
        }
        MarketAsset::Quote => {
            Debt::debt_to_shares(amounts.debt_amount, market.debt.base_borrow_index_nad)?
        }
    };
    match target_asset {
        MarketAsset::Base => {
            market.base_hlp_vault.add_debt_shares(debt_shares)?;
            market
                .base_hlp_vault
                .add_debt_principal(amounts.debt_amount)?;
            market
                .base_hlp_vault
                .credit_ylp(target_ylp_amount, ylp_amount)?;
        }
        MarketAsset::Quote => {
            market.quote_hlp_vault.add_debt_shares(debt_shares)?;
            market
                .quote_hlp_vault
                .add_debt_principal(amounts.debt_amount)?;
            market
                .quote_hlp_vault
                .credit_ylp(ylp_amount, target_ylp_amount)?;
        }
    }
    let executed_delta =
        executed_delta_for_borrowed_amount(market, target_asset, amounts.debt_amount)?;
    Ok(HlpRebalanceReceipt {
        target_asset,
        ideal_delta,
        executed_delta,
        base_ylp_mint_amount: if target_asset == MarketAsset::Base {
            target_ylp_amount
        } else if borrowed_asset == MarketAsset::Base {
            ylp_amount
        } else {
            0
        },
        quote_ylp_mint_amount: if target_asset == MarketAsset::Quote {
            target_ylp_amount
        } else if borrowed_asset == MarketAsset::Quote {
            ylp_amount
        } else {
            0
        },
        debt_delta: amounts.debt_amount as i128,
        ..HlpRebalanceReceipt::default()
    })
}

fn feasible_leverage_up_target_amount(
    market: &Market,
    target_asset: MarketAsset,
    requested_delta_nad: u128,
) -> Result<u64> {
    let requested_target_amount =
        target_raw_amount_from_delta(market, target_asset, requested_delta_nad)?;
    let borrow_headroom = market.side(target_asset.opposite())?.reserves.cash_reserve;
    if borrow_headroom == 0 {
        return Ok(0);
    }
    let headroom_value_nad = asset_value_in_target_nad(
        market,
        target_asset.opposite(),
        borrow_headroom,
        target_asset,
    )?;
    let headroom_target_amount =
        target_raw_amount_from_delta(market, target_asset, headroom_value_nad)?;
    Ok(requested_target_amount.min(headroom_target_amount))
}

fn deleverage_balanced(
    market: &mut Market,
    target_asset: MarketAsset,
    ideal_delta: i128,
) -> Result<HlpRebalanceReceipt> {
    let borrowed_asset = target_asset.opposite();

    let (borrow_index, debt_shares, vault_target_ylp, vault_borrowed_ylp) = match target_asset {
        MarketAsset::Base => (
            market.debt.quote_borrow_index_nad,
            market.base_hlp_vault.debt_shares,
            market.base_hlp_vault.ylp_base_shares,
            market.base_hlp_vault.ylp_quote_shares,
        ),
        MarketAsset::Quote => (
            market.debt.base_borrow_index_nad,
            market.quote_hlp_vault.debt_shares,
            market.quote_hlp_vault.ylp_quote_shares,
            market.quote_hlp_vault.ylp_base_shares,
        ),
    };
    let current_debt = Debt::shares_to_debt(debt_shares, borrow_index)?;
    let current_debt = u64::try_from(current_debt).unwrap_or(u64::MAX);
    let target_side = market.side(target_asset)?;
    let borrowed_side = market.side(borrowed_asset)?;
    let target_underlying = ylp_underlying_amount(target_side, vault_target_ylp)?;
    let borrowed_underlying = ylp_underlying_amount(borrowed_side, vault_borrowed_ylp)?;
    let target_total_amount = feasible_deleverage_target_amount(
        market,
        target_asset,
        ideal_delta.unsigned_abs(),
        target_underlying,
        borrowed_underlying,
        current_debt,
    )?;
    let amounts =
        balanced_rebalance_amounts_from_target_amount(market, target_asset, target_total_amount)?;
    if amounts.target_leg_amount == 0
        || amounts.borrowed_leg_amount == 0
        || amounts.debt_amount == 0
    {
        return Ok(HlpRebalanceReceipt {
            target_asset,
            ideal_delta,
            ..HlpRebalanceReceipt::default()
        });
    }

    let target_side = market.side_mut(target_asset)?;
    let target_ylp_burn = ylp_shares_for_reserve_amount(target_side, amounts.target_leg_amount)?
        .min(vault_target_ylp);
    require!(target_ylp_burn > 0, ErrorCode::AmountZero);
    debit_reserve(target_side, amounts.target_leg_amount, false)?;
    target_side.shares.burn(target_ylp_burn)?;
    target_side.assert_share_backing()?;

    let borrowed_side = market.side_mut(borrowed_asset)?;
    let ylp_burn = ylp_shares_for_reserve_amount(borrowed_side, amounts.borrowed_leg_amount)?
        .min(vault_borrowed_ylp);
    require!(ylp_burn > 0, ErrorCode::AmountZero);
    debit_reserve(borrowed_side, amounts.borrowed_leg_amount, false)?;
    borrowed_side.shares.burn(ylp_burn)?;
    borrowed_side.assert_share_backing()?;

    let repay_amount = amounts.debt_amount.min(current_debt);
    let debt_shares_to_remove = Debt::debt_to_shares(repay_amount, borrow_index)?.min(debt_shares);
    match target_asset {
        MarketAsset::Base => {
            let _interest_paid = market
                .base_hlp_vault
                .realize_debt_repay(repay_amount, borrow_index)?;
            market
                .base_hlp_vault
                .remove_debt_shares(debt_shares_to_remove)?;
            market.base_hlp_vault.debit_ylp(target_ylp_burn, ylp_burn)?;
        }
        MarketAsset::Quote => {
            let _interest_paid = market
                .quote_hlp_vault
                .realize_debt_repay(repay_amount, borrow_index)?;
            market
                .quote_hlp_vault
                .remove_debt_shares(debt_shares_to_remove)?;
            market
                .quote_hlp_vault
                .debit_ylp(ylp_burn, target_ylp_burn)?;
        }
    }
    let executed_abs = executed_delta_for_borrowed_amount(market, target_asset, repay_amount)?;
    Ok(HlpRebalanceReceipt {
        target_asset,
        ideal_delta,
        executed_delta: -executed_abs,
        base_ylp_burn_amount: if target_asset == MarketAsset::Base {
            target_ylp_burn
        } else if borrowed_asset == MarketAsset::Base {
            ylp_burn
        } else {
            0
        },
        quote_ylp_burn_amount: if target_asset == MarketAsset::Quote {
            target_ylp_burn
        } else if borrowed_asset == MarketAsset::Quote {
            ylp_burn
        } else {
            0
        },
        debt_delta: -(repay_amount as i128),
        ..HlpRebalanceReceipt::default()
    })
}

fn balanced_rebalance_amounts_from_target_amount(
    market: &Market,
    target_asset: MarketAsset,
    target_total_amount: u64,
) -> Result<BalancedRebalanceAmounts> {
    let target_leg_amount = target_total_amount / 2;
    if target_leg_amount == 0 {
        return Ok(BalancedRebalanceAmounts::default());
    }
    let borrowed_leg_amount = market.spot_value_in_opposite(target_asset, target_leg_amount)?;
    let debt_amount = market.spot_value_in_opposite(target_asset, target_total_amount)?;
    Ok(BalancedRebalanceAmounts {
        target_leg_amount,
        borrowed_leg_amount,
        debt_amount,
    })
}

fn feasible_deleverage_target_amount(
    market: &Market,
    target_asset: MarketAsset,
    requested_delta_nad: u128,
    target_underlying: u64,
    borrowed_underlying: u64,
    current_debt: u64,
) -> Result<u64> {
    let requested_target_amount =
        target_raw_amount_from_delta(market, target_asset, requested_delta_nad)?;
    let target_cap = target_underlying
        .checked_mul(2)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let borrowed_value_nad = asset_value_in_target_nad(
        market,
        target_asset.opposite(),
        borrowed_underlying,
        target_asset,
    )?;
    let borrowed_cap = target_raw_amount_from_delta(market, target_asset, borrowed_value_nad)?
        .checked_mul(2)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let debt_value_nad =
        asset_value_in_target_nad(market, target_asset.opposite(), current_debt, target_asset)?;
    let debt_cap = target_raw_amount_from_delta(market, target_asset, debt_value_nad)?;
    Ok(requested_target_amount
        .min(target_cap)
        .min(borrowed_cap)
        .min(debt_cap))
}

fn refresh_hlp_after_rebalance(
    market: &mut Market,
    target_asset: MarketAsset,
    current_slot: u64,
    mut receipt: HlpRebalanceReceipt,
) -> Result<HlpRebalanceReceipt> {
    let nav = hlp_nav_nad(market, target_asset)?;
    let settlement_price = current_settlement_price_nad(market, target_asset)?;
    let pending_rebalance = receipt
        .ideal_delta
        .checked_sub(receipt.executed_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let vault = match target_asset {
        MarketAsset::Base => &mut market.base_hlp_vault,
        MarketAsset::Quote => &mut market.quote_hlp_vault,
    };
    vault.last_nav_nad = nav;
    vault.pending_rebalance = pending_rebalance;
    vault.cached_settlement_price_nad = settlement_price;
    vault.last_rebalance_slot = current_slot;
    receipt.pending_rebalance = pending_rebalance;
    receipt.nav_nad = nav;
    Ok(receipt)
}

fn target_raw_amount_from_delta(
    market: &Market,
    target_asset: MarketAsset,
    delta_nad: u128,
) -> Result<u64> {
    let decimals = market.side(target_asset)?.asset_decimals;
    denormalize_from_nad_floor(delta_nad, decimals)
}

fn executed_delta_for_borrowed_amount(
    market: &Market,
    target_asset: MarketAsset,
    borrowed_amount: u64,
) -> Result<i128> {
    let value = asset_value_in_target_nad(
        market,
        target_asset.opposite(),
        borrowed_amount,
        target_asset,
    )?;
    i128::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn ylp_shares_for_reserve_amount(
    side: &crate::state::MarketSide,
    reserve_amount: u64,
) -> Result<u64> {
    if reserve_amount == 0 {
        return Ok(0);
    }
    require!(
        side.reserves.live_reserve > 0 && side.shares.ylp_supply > 0,
        ErrorCode::InsufficientLiquidity
    );
    let shares = ceil_div(
        (reserve_amount as u128)
            .checked_mul(side.shares.ylp_supply as u128)
            .ok_or(ErrorCode::MarketMathOverflow)?,
        side.reserves.live_reserve as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(shares).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn require_hlp_borrow_headroom(side: &crate::state::MarketSide, amount: u64) -> Result<()> {
    require_gte!(
        side.reserves.cash_reserve,
        amount,
        ErrorCode::InsufficientBorrowHeadroom
    );
    Ok(())
}

fn checkpoint_one_hlp(
    market: &mut Market,
    target_asset: MarketAsset,
    current_slot: u64,
) -> Result<i128> {
    checkpoint_hlp_yield_from_ylp(market, target_asset)?;
    let nav = hlp_nav_nad(market, target_asset)?;
    let settlement_price = current_settlement_price_nad(market, target_asset)?;
    let (collateral, debt, vault) = match target_asset {
        MarketAsset::Base => {
            let collateral =
                hlp_collateral_value_nad(market, MarketAsset::Base, &market.base_hlp_vault)?;
            let debt = hlp_debt_value_nad(market, MarketAsset::Base)?;
            (collateral, debt, &mut market.base_hlp_vault)
        }
        MarketAsset::Quote => {
            let collateral =
                hlp_collateral_value_nad(market, MarketAsset::Quote, &market.quote_hlp_vault)?;
            let debt = hlp_debt_value_nad(market, MarketAsset::Quote)?;
            (collateral, debt, &mut market.quote_hlp_vault)
        }
    };
    let ideal_delta = (collateral as i128)
        .checked_sub(debt.checked_mul(2).ok_or(ErrorCode::DebtMathOverflow)? as i128)
        .ok_or(ErrorCode::DebtMathOverflow)?;
    vault.last_nav_nad = nav;
    vault.pending_rebalance = ideal_delta;
    vault.cached_settlement_price_nad = settlement_price;
    vault.last_rebalance_slot = current_slot;
    Ok(ideal_delta)
}

pub fn checkpoint_hlp_yield_from_ylp(market: &mut Market, target_asset: MarketAsset) -> Result<()> {
    carry_forward_swap_fees(&mut market.base_side)?;
    carry_forward_interest(&mut market.base_side)?;
    carry_forward_swap_fees(&mut market.quote_side)?;
    carry_forward_interest(&mut market.quote_side)?;
    let base_side = market.base_side;
    let quote_side = market.quote_side;
    match target_asset {
        MarketAsset::Base => market
            .base_hlp_vault
            .checkpoint_yield_from_ylp(&base_side, &quote_side),
        MarketAsset::Quote => market
            .quote_hlp_vault
            .checkpoint_yield_from_ylp(&base_side, &quote_side),
    }
}

fn require_hlp_settlement_available(market: &Market, target_asset: MarketAsset) -> Result<()> {
    let vault = match target_asset {
        MarketAsset::Base => &market.base_hlp_vault,
        MarketAsset::Quote => &market.quote_hlp_vault,
    };
    if vault.hlp_supply == 0 || vault.cached_settlement_price_nad == 0 {
        return Ok(());
    }
    let current_price = current_settlement_price_nad(market, target_asset)?;
    let reference_price = vault.cached_settlement_price_nad;
    let divergence = if current_price >= reference_price {
        current_price
            .checked_sub(reference_price)
            .ok_or(ErrorCode::MarketMathOverflow)?
    } else {
        reference_price
            .checked_sub(current_price)
            .ok_or(ErrorCode::MarketMathOverflow)?
    };
    let max_divergence = reference_price
        .checked_mul(market.config.settlement_divergence_bps as u128)
        .and_then(|value| value.checked_div(crate::constants::BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    require!(
        divergence <= max_divergence,
        ErrorCode::HlpSettlementUnavailable
    );
    Ok(())
}

fn current_settlement_price_nad(market: &Market, target_asset: MarketAsset) -> Result<u128> {
    match target_asset {
        MarketAsset::Base => {
            market_spot_price_nad(&market.base_side, &market.quote_side).map(u128::from)
        }
        MarketAsset::Quote => {
            market_spot_price_nad(&market.quote_side, &market.base_side).map(u128::from)
        }
    }
}

fn hlp_nav_nad(market: &Market, target_asset: MarketAsset) -> Result<u128> {
    let (collateral, debt) = match target_asset {
        MarketAsset::Base => (
            hlp_collateral_value_nad(market, MarketAsset::Base, &market.base_hlp_vault)?,
            hlp_debt_value_nad(market, MarketAsset::Base)?,
        ),
        MarketAsset::Quote => (
            hlp_collateral_value_nad(market, MarketAsset::Quote, &market.quote_hlp_vault)?,
            hlp_debt_value_nad(market, MarketAsset::Quote)?,
        ),
    };
    collateral
        .checked_sub(debt)
        .ok_or(ErrorCode::Undercollateralized.into())
}

fn hlp_collateral_value_nad(
    market: &Market,
    target_asset: MarketAsset,
    vault: &HlpVault,
) -> Result<u128> {
    let base_underlying = ylp_underlying_amount(&market.base_side, vault.ylp_base_shares)?;
    let quote_underlying = ylp_underlying_amount(&market.quote_side, vault.ylp_quote_shares)?;
    let base_value =
        asset_value_in_target_nad(market, MarketAsset::Base, base_underlying, target_asset)?;
    let quote_value =
        asset_value_in_target_nad(market, MarketAsset::Quote, quote_underlying, target_asset)?;
    base_value
        .checked_add(quote_value)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn hlp_debt_value_nad(market: &Market, target_asset: MarketAsset) -> Result<u128> {
    let (borrowed_asset, debt_amount) = match target_asset {
        MarketAsset::Base => (
            MarketAsset::Quote,
            Debt::shares_to_debt(
                market.base_hlp_vault.debt_shares,
                market.debt.quote_borrow_index_nad,
            )?,
        ),
        MarketAsset::Quote => (
            MarketAsset::Base,
            Debt::shares_to_debt(
                market.quote_hlp_vault.debt_shares,
                market.debt.base_borrow_index_nad,
            )?,
        ),
    };
    let debt_amount = u64::try_from(debt_amount).map_err(|_| ErrorCode::DebtMathOverflow)?;
    asset_value_in_target_nad(market, borrowed_asset, debt_amount, target_asset)
}

fn ylp_underlying_amount(side: &crate::state::MarketSide, ylp_amount: u64) -> Result<u64> {
    if ylp_amount == 0 || side.shares.ylp_supply == 0 {
        return Ok(0);
    }
    let reserve_amount = (ylp_amount as u128)
        .checked_mul(side.reserves.live_reserve as u128)
        .and_then(|value| value.checked_div(side.shares.ylp_supply as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(reserve_amount).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn asset_value_in_target_nad(
    market: &Market,
    asset: MarketAsset,
    amount: u64,
    target_asset: MarketAsset,
) -> Result<u128> {
    let asset_side = market.side(asset)?;
    let amount_nad = normalize_to_nad(amount as u128, asset_side.asset_decimals)?;
    if asset == target_asset {
        return Ok(amount_nad);
    }
    let target_side = market.side(target_asset)?;
    let price_nad = market_spot_price_nad(asset_side, target_side)? as u128;
    amount_nad
        .checked_mul(price_nad)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn proportional(amount: u64, numerator: u64, denominator: u64) -> Result<u64> {
    let value = (amount as u128)
        .checked_mul(numerator as u128)
        .and_then(|value| value.checked_div(denominator as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

fn proportional_u128(amount: u128, numerator: u64, denominator: u64) -> Result<u128> {
    amount
        .checked_mul(numerator as u128)
        .and_then(|value| value.checked_div(denominator as u128))
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn hlp_shares_for_delta_nav(
    delta_nav_nad: u128,
    nav_basis_nad: u128,
    hlp_supply: u64,
) -> Result<u64> {
    require!(delta_nav_nad > 0, ErrorCode::AmountZero);
    require!(nav_basis_nad > 0, ErrorCode::MarketMathOverflow);
    let shares = delta_nav_nad
        .checked_mul(hlp_supply as u128)
        .and_then(|value| value.checked_div(nav_basis_nad))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let shares = u64::try_from(shares).map_err(|_| ErrorCode::MarketMathOverflow)?;
    require!(shares > 0, ErrorCode::AmountZero);
    Ok(shares)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, MARKET_VERSION},
        math::calculate_raw_amount_out,
        state::{Insurance, MarketConfig, MarketHealth, MarketSide, Risk},
        transitions::swap::Swap as SwapTransition,
    };

    fn valid_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 30,
            manager_fee_bps: 0,
            protocol_fee_bps: 0,
            target_hlp_leverage_bps: BPS_DENOMINATOR * 2,
            settlement_divergence_bps: 500,
            emergency_exit_haircut_bps: 250,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: 2_000,
            max_daily_withdraw_bps: 2_000,
            spot_ema_divergence_bps: 1_000,
            k_ema_drawdown_bps: 1_000,
            recognized_collateral_cap_bps: 15_000,
            market_health_min_bps: 11_000,
            soft_borrow_enabled: false,
            hedged_lp_enabled: true,
            start_time: 0,
        }
    }

    fn seeded_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = MarketSide {
            asset_mint: base_mint,
            asset_decimals: 0,
            ..MarketSide::default()
        };
        base_side.reserves.live_reserve = 1_000;
        base_side.reserves.cash_reserve = 1_000;
        base_side.shares.ylp_supply = 1_000;

        let mut quote_side = MarketSide {
            asset_mint: quote_mint,
            asset_decimals: 0,
            ..MarketSide::default()
        };
        quote_side.reserves.live_reserve = 2_000;
        quote_side.reserves.cash_reserve = 2_000;
        quote_side.shares.ylp_supply = 2_000;

        let mut base_hlp_vault = HlpVault::default();
        base_hlp_vault.initialize(
            MarketAsset::Base,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
        );
        let mut quote_hlp_vault = HlpVault::default();
        quote_hlp_vault.initialize(
            MarketAsset::Quote,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
        );

        Market {
            version: MARKET_VERSION,
            base_mint,
            quote_mint,
            operator: Pubkey::new_unique(),
            manager: Pubkey::new_unique(),
            base_side,
            quote_side,
            config: valid_config(),
            debt: Debt {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                ..Debt::default()
            },
            base_hlp_vault,
            quote_hlp_vault,
            risk: Risk::default(),
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            params_hash: [7; 32],
            last_update_slot: 0,
            reduce_only: false,
            bump: 255,
        }
    }

    #[test]
    fn open_hlp_keeps_leverage_debt_on_aggregate_vault() {
        let mut market = seeded_market();

        let receipt = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();

        assert_eq!(receipt.borrowed_amount, 200);
        assert_eq!(receipt.base_ylp_amount, 100);
        assert_eq!(receipt.quote_ylp_amount, 200);
        assert_eq!(receipt.hlp_amount, 100);
        assert_eq!(market.debt.fixed_quote_shares, 0);
        assert!(market.base_hlp_vault.debt_shares > 0);
        assert_eq!(market.base_hlp_vault.debt_principal, 200);
        assert_eq!(market.base_hlp_vault.ylp_base_shares, 100);
        assert_eq!(market.base_hlp_vault.ylp_quote_shares, 200);
        assert_eq!(market.base_side.reserves.cash_reserve, 1_100);
        assert_eq!(market.quote_side.reserves.cash_reserve, 2_000);
        assert_eq!(market.base_hlp_vault.last_nav_nad, 100 * NAD as u128);
    }

    #[test]
    fn open_hlp_requires_borrowed_side_cash_headroom() {
        let mut market = seeded_market();
        market.quote_side.reserves.cash_reserve = 199;

        let err = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientBorrowHeadroom));
    }

    #[test]
    fn repeated_open_hlp_mints_against_delta_nav() {
        let mut market = seeded_market();

        let first = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        let second = OpenHedge::new(MarketAsset::Base, 120, 1)
            .apply(&mut market)
            .unwrap();

        assert_eq!(first.hlp_amount, 100);
        assert_eq!(second.hlp_amount, 120);
        assert_eq!(market.base_hlp_vault.hlp_supply, 220);
        assert_eq!(market.base_hlp_vault.ylp_base_shares, 220);
        assert_eq!(market.base_hlp_vault.ylp_quote_shares, 440);
        assert_eq!(market.base_hlp_vault.last_nav_nad, 220 * NAD as u128);
    }

    #[test]
    fn h_lp_nav_values_collateral_and_debt_in_target_numeraire() {
        let mut market = seeded_market();

        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();

        assert_eq!(
            hlp_collateral_value_nad(&market, MarketAsset::Base, &market.base_hlp_vault).unwrap(),
            200 * NAD as u128
        );
        assert_eq!(
            hlp_debt_value_nad(&market, MarketAsset::Base).unwrap(),
            100 * NAD as u128
        );
        assert_eq!(
            hlp_nav_nad(&market, MarketAsset::Base).unwrap(),
            100 * NAD as u128
        );
    }

    #[test]
    fn accrued_interest_grows_hlp_debt_and_reduces_nav() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        let debt_before = hlp_debt_value_nad(&market, MarketAsset::Base).unwrap();
        let nav_before = hlp_nav_nad(&market, MarketAsset::Base).unwrap();

        // Simulate 10% borrow-interest accrual on the quote index. The base-hLP
        // borrows quote, so its debt grows and its NAV falls one-for-one: this is
        // how interest is charged to the hedged-LP position.
        market.debt.quote_borrow_index_nad = (NAD as u128) * 110 / 100;

        let debt_after = hlp_debt_value_nad(&market, MarketAsset::Base).unwrap();
        let nav_after = hlp_nav_nad(&market, MarketAsset::Base).unwrap();
        assert_eq!(debt_after, debt_before * 110 / 100);
        assert_eq!(nav_after, nav_before - (debt_after - debt_before));
        assert_eq!(market.base_hlp_vault.debt_principal, 200);
        assert_eq!(debt_after, 110 * NAD as u128);
        assert_eq!(nav_after, 90 * NAD as u128);
    }

    #[test]
    fn close_hlp_burns_vault_ylp_and_repays_vault_debt() {
        let mut market = seeded_market();
        let open_receipt = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();

        let close_receipt = CloseHedge::new(MarketAsset::Base, open_receipt.hlp_amount)
            .apply(&mut market)
            .unwrap();

        assert_eq!(close_receipt.target_amount_out, 100);
        assert_eq!(close_receipt.debt_repaid, 200);
        assert_eq!(market.base_hlp_vault.hlp_supply, 0);
        assert_eq!(market.base_hlp_vault.debt_shares, 0);
        assert_eq!(market.base_hlp_vault.debt_principal, 0);
        assert_eq!(market.base_hlp_vault.ylp_base_shares, 0);
        assert_eq!(market.base_hlp_vault.ylp_quote_shares, 0);
        assert_eq!(market.debt.fixed_quote_shares, 0);
        assert_eq!(market.base_side.reserves.live_reserve, 1_000);
        assert_eq!(market.base_side.reserves.cash_reserve, 1_000);
        assert_eq!(market.quote_side.reserves.live_reserve, 2_000);
        assert_eq!(market.quote_side.reserves.cash_reserve, 2_000);
        assert_eq!(market.base_side.shares.ylp_supply, 1_000);
        assert_eq!(market.quote_side.shares.ylp_supply, 2_000);
    }

    #[test]
    fn close_hlp_converts_borrowed_side_surplus_into_target_out() {
        let mut market = seeded_market();
        let open_receipt = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_300;

        let close_receipt = CloseHedge::new(MarketAsset::Base, open_receipt.hlp_amount)
            .apply(&mut market)
            .unwrap();

        assert!(close_receipt.target_amount_out > 100);
        assert_eq!(close_receipt.debt_repaid, 200);
        assert_eq!(market.base_hlp_vault.hlp_supply, 0);
        assert_eq!(market.quote_side.reserves.live_reserve, 2_100);
    }

    #[test]
    fn close_hlp_uses_target_side_value_for_borrowed_side_shortfall() {
        let mut market = seeded_market();
        let open_receipt = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_110;

        let close_receipt = CloseHedge::new(MarketAsset::Base, open_receipt.hlp_amount)
            .apply(&mut market)
            .unwrap();

        assert!(close_receipt.target_amount_out < 100);
        assert_eq!(close_receipt.debt_repaid, 200);
        assert_eq!(market.base_hlp_vault.hlp_supply, 0);
        assert_eq!(market.quote_side.reserves.live_reserve, 1_910);
    }

    #[test]
    fn open_hlp_rejects_settlement_price_divergence() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();

        market.quote_side.reserves.live_reserve = 4_000;
        market.quote_side.reserves.cash_reserve = 4_000;
        let err = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::HlpSettlementUnavailable));
    }

    #[test]
    fn close_hlp_rejects_settlement_price_divergence() {
        let mut market = seeded_market();
        let receipt = OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();

        market.quote_side.reserves.live_reserve = 4_000;
        market.quote_side.reserves.cash_reserve = 4_000;
        let err = CloseHedge::new(MarketAsset::Base, receipt.hlp_amount)
            .apply(&mut market)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::HlpSettlementUnavailable));
    }

    #[test]
    fn h_lp_checkpoint_refreshes_settlement_reference() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_080;
        market.quote_side.reserves.cash_reserve = 2_080;

        checkpoint_hlp_vaults(&mut market, 42).unwrap();

        assert_eq!(market.base_hlp_vault.last_rebalance_slot, 42);
        assert_eq!(
            market.base_hlp_vault.cached_settlement_price_nad,
            current_settlement_price_nad(&market, MarketAsset::Base).unwrap()
        );
    }

    fn assert_hlp_near_target(market: &Market, target_asset: MarketAsset, max_gap_nad: u128) {
        let gap = current_hlp_ideal_delta(market, target_asset).unwrap();
        assert!(
            gap.unsigned_abs() <= max_gap_nad,
            "hLP target gap {} exceeds {}",
            gap,
            max_gap_nad
        );
    }

    #[test]
    fn rebalance_hlp_leverages_up_with_balanced_ylp() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_400;
        let base_ylp_before = market.base_hlp_vault.ylp_base_shares;
        let quote_ylp_before = market.base_hlp_vault.ylp_quote_shares;
        let debt_before = market.base_hlp_vault.debt_shares;
        let principal_before = market.base_hlp_vault.debt_principal;

        let (base_receipt, _) = rebalance_hlp_vaults(&mut market, 43).unwrap();

        assert!(base_receipt.ideal_delta > 0);
        assert!(base_receipt.executed_delta > 0);
        assert!(base_receipt.base_ylp_mint_amount > 0);
        assert!(base_receipt.quote_ylp_mint_amount > 0);
        assert_eq!(base_receipt.base_ylp_burn_amount, 0);
        assert_eq!(base_receipt.quote_ylp_burn_amount, 0);
        assert!(market.base_hlp_vault.ylp_base_shares > base_ylp_before);
        assert!(market.base_hlp_vault.ylp_quote_shares > 200);
        assert!(market.base_hlp_vault.ylp_quote_shares > quote_ylp_before);
        assert!(market.base_hlp_vault.debt_shares > debt_before);
        assert!(market.base_hlp_vault.debt_principal > principal_before);
        assert_eq!(market.base_hlp_vault.last_rebalance_slot, 43);
        assert_eq!(
            market.base_hlp_vault.pending_rebalance,
            base_receipt.pending_rebalance
        );
        assert_hlp_near_target(&market, MarketAsset::Base, 2 * NAD as u128);
    }

    #[test]
    fn rebalance_hlp_leverage_up_stores_pending_when_borrow_cash_is_constrained() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_400;
        market.quote_side.reserves.cash_reserve = 5;
        let ideal_before = current_hlp_ideal_delta(&market, MarketAsset::Base).unwrap();
        assert!(ideal_before > 0);

        let (base_receipt, _) = rebalance_hlp_vaults(&mut market, 47).unwrap();

        assert!(base_receipt.executed_delta > 0);
        assert!(base_receipt.executed_delta < ideal_before);
        assert!(base_receipt.pending_rebalance > 0);
        assert!(base_receipt.debt_delta > 0);
        assert!(base_receipt.debt_delta <= 5);
        assert_eq!(
            market.base_hlp_vault.pending_rebalance,
            base_receipt.pending_rebalance
        );
    }

    #[test]
    fn rebalance_hlp_leverage_up_keeps_swap_live_without_borrow_cash() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 2_400;
        market.quote_side.reserves.cash_reserve = 0;
        let ideal_before = current_hlp_ideal_delta(&market, MarketAsset::Base).unwrap();
        assert!(ideal_before > 0);

        let (base_receipt, _) = rebalance_hlp_vaults(&mut market, 48).unwrap();

        assert_eq!(base_receipt.executed_delta, 0);
        assert_eq!(base_receipt.pending_rebalance, ideal_before);
        assert_eq!(base_receipt.debt_delta, 0);
        assert_eq!(market.base_hlp_vault.pending_rebalance, ideal_before);
    }

    #[test]
    fn rebalance_hlp_deleverages_with_balanced_ylp() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Base, 100, 1)
            .apply(&mut market)
            .unwrap();
        market.quote_side.reserves.live_reserve = 1_800;
        let base_ylp_before = market.base_hlp_vault.ylp_base_shares;
        let quote_ylp_before = market.base_hlp_vault.ylp_quote_shares;
        let debt_before = market.base_hlp_vault.debt_shares;
        let principal_before = market.base_hlp_vault.debt_principal;

        let (base_receipt, _) = rebalance_hlp_vaults(&mut market, 44).unwrap();

        assert!(base_receipt.ideal_delta < 0);
        assert!(base_receipt.executed_delta < 0);
        assert!(base_receipt.base_ylp_burn_amount > 0);
        assert!(base_receipt.quote_ylp_burn_amount > 0);
        assert_eq!(base_receipt.base_ylp_mint_amount, 0);
        assert_eq!(base_receipt.quote_ylp_mint_amount, 0);
        assert!(market.base_hlp_vault.ylp_base_shares < base_ylp_before);
        assert!(market.base_hlp_vault.ylp_quote_shares < quote_ylp_before);
        assert!(market.base_hlp_vault.debt_shares < debt_before);
        assert!(market.base_hlp_vault.debt_principal < principal_before);
        assert_eq!(market.base_hlp_vault.last_rebalance_slot, 44);
        assert_eq!(
            market.base_hlp_vault.pending_rebalance,
            base_receipt.pending_rebalance
        );
        assert_hlp_near_target(&market, MarketAsset::Base, 2 * NAD as u128);
    }

    #[test]
    fn quote_hlp_rebalance_moves_both_ylp_sides() {
        let mut market = seeded_market();
        OpenHedge::new(MarketAsset::Quote, 200, 1)
            .apply(&mut market)
            .unwrap();
        market.base_side.reserves.live_reserve = 1_200;
        let base_ylp_before = market.quote_hlp_vault.ylp_base_shares;
        let quote_ylp_before = market.quote_hlp_vault.ylp_quote_shares;
        let debt_before = market.quote_hlp_vault.debt_shares;
        let principal_before = market.quote_hlp_vault.debt_principal;

        let (_, quote_receipt) = rebalance_hlp_vaults(&mut market, 45).unwrap();

        assert!(quote_receipt.ideal_delta > 0);
        assert!(quote_receipt.executed_delta > 0);
        assert!(quote_receipt.base_ylp_mint_amount > 0);
        assert!(quote_receipt.quote_ylp_mint_amount > 0);
        assert!(market.quote_hlp_vault.ylp_base_shares > base_ylp_before);
        assert!(market.quote_hlp_vault.ylp_quote_shares > quote_ylp_before);
        assert!(market.quote_hlp_vault.debt_shares > debt_before);
        assert!(market.quote_hlp_vault.debt_principal > principal_before);
        assert_eq!(market.quote_hlp_vault.last_rebalance_slot, 45);
        assert_hlp_near_target(&market, MarketAsset::Quote, 5 * NAD as u128);
    }

    #[test]
    fn swap_rebalance_is_price_neutral_after_user_quote() {
        let mut market = seeded_market();
        market.base_side.reserves.live_reserve = 1_000_000;
        market.base_side.reserves.cash_reserve = 1_000_000;
        market.base_side.shares.ylp_supply = 1_000_000;
        market.quote_side.reserves.live_reserve = 2_000_000;
        market.quote_side.reserves.cash_reserve = 2_000_000;
        market.quote_side.shares.ylp_supply = 2_000_000;

        OpenHedge::new(MarketAsset::Base, 100_000, 1)
            .apply(&mut market)
            .unwrap();
        OpenHedge::new(MarketAsset::Quote, 200_000, 1)
            .apply(&mut market)
            .unwrap();

        let amount_in_after_fee = 50_000;
        let amount_out = calculate_raw_amount_out(
            market.base_side.reserves.live_reserve,
            market.quote_side.reserves.live_reserve,
            amount_in_after_fee,
        )
        .unwrap();
        let (market_side_in, market_side_out) = market.swap_sides_mut(MarketAsset::Base);
        SwapTransition::new(
            amount_in_after_fee,
            amount_out,
            0,
            0,
            0,
            crate::state::ProtocolAuctionSplit::default(),
        )
        .apply(market_side_in, market_side_out)
        .unwrap();

        let quoted_post_swap_price =
            market_spot_price_nad(&market.base_side, &market.quote_side).unwrap();
        let base_liquidity_before = market.base_side.reserves.live_reserve;
        let quote_liquidity_before = market.quote_side.reserves.live_reserve;

        let (base_receipt, quote_receipt) = rebalance_hlp_vaults(&mut market, 46).unwrap();

        assert!(
            base_receipt.executed_delta != 0 || quote_receipt.executed_delta != 0,
            "test must exercise an hLP rebalance"
        );
        assert_ne!(
            market.base_side.reserves.live_reserve,
            base_liquidity_before
        );
        assert_ne!(
            market.quote_side.reserves.live_reserve,
            quote_liquidity_before
        );

        let post_rebalance_price =
            market_spot_price_nad(&market.base_side, &market.quote_side).unwrap();
        let price_diff = quoted_post_swap_price.abs_diff(post_rebalance_price);
        assert!(
            price_diff <= quoted_post_swap_price / BPS_DENOMINATOR as u64 + 1,
            "hLP rebalance moved post-swap spot by more than rounding: quoted {}, final {}",
            quoted_post_swap_price,
            post_rebalance_price
        );
    }
}
