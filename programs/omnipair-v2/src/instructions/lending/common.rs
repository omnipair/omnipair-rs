use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::{
    errors::ErrorCode,
    state::{DebtBook, MarginPosition, Market, MarketSide},
    utils::market_math::require_market_reserve_floor,
};

pub(super) fn validate_collateral_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        owner_asset_account.mint,
        asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_asset_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

pub(super) fn validate_borrow_accounts<'info>(
    market: &Account<'info, Market>,
    borrow_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (debt_side, collateral_side) = if borrow_asset_is_asset0 {
        (&market.side0, &market.side1)
    } else {
        (&market.side1, &market.side0)
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )?;
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    Ok(())
}

pub(super) fn validate_repay_accounts<'info>(
    market: &Account<'info, Market>,
    repay_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let debt_side = if repay_asset_is_asset0 {
        &market.side0
    } else {
        &market.side1
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )
}

fn validate_debt_reserve_accounts<'info>(
    market: &Account<'info, Market>,
    debt_side: &MarketSide,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_debt_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

pub(super) fn apply_borrow_state(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    borrow_asset_is_asset0: bool,
    borrow_amount: u64,
    min_health_bps: u64,
) -> Result<()> {
    let debt_shares = if borrow_asset_is_asset0 {
        DebtBook::debt_to_shares(borrow_amount, market.debt_book.borrow_index0_nad)?
    } else {
        DebtBook::debt_to_shares(borrow_amount, market.debt_book.borrow_index1_nad)?
    };
    let debt_side_index = if borrow_asset_is_asset0 { 0 } else { 1 };
    market.enforce_daily_borrow_limit(debt_side_index, borrow_amount)?;
    let debt_side = if borrow_asset_is_asset0 {
        &mut market.side0
    } else {
        &mut market.side1
    };
    require_borrow_headroom(debt_side, borrow_amount)?;
    debt_side.reserve_ledger.live_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    debt_side.reserve_ledger.cash_reserve = debt_side
        .reserve_ledger
        .cash_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::CashReserveUnderflow)?;

    if borrow_asset_is_asset0 {
        margin_position.fixed_debt0_shares = margin_position
            .fixed_debt0_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_debt0_shares = market
            .debt_book
            .fixed_debt0_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflow)?;
    } else {
        margin_position.fixed_debt1_shares = margin_position
            .fixed_debt1_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_debt1_shares = market
            .debt_book
            .fixed_debt1_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflow)?;
    }
    sync_borrow_recognition(market, margin_position, borrow_asset_is_asset0)?;
    market.refresh_market_health()?;
    market.assert_market_health()?;
    market.assert_risk_circuit_breakers()?;
    market.assert_recognition_cap(margin_position, borrow_asset_is_asset0)?;
    market.assert_position_health(margin_position, borrow_asset_is_asset0, min_health_bps)?;
    let health = if borrow_asset_is_asset0 {
        market.position_health_bps(margin_position, true)?
    } else {
        market.position_health_bps(margin_position, false)?
    };
    require_gte!(health, min_health_bps, ErrorCode::InsufficientMarketHealth);
    Ok(())
}

fn sync_borrow_recognition(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset_is_asset0: bool,
) -> Result<()> {
    let risk_book = market.current_risk_book()?;
    let recognition_slot = Clock::get()
        .map(|clock| clock.slot)
        .unwrap_or(market.last_update_slot);

    if debt_asset_is_asset0 {
        let old_recognized = margin_position.recognized_collateral1_for_debt0;
        let target_recognized =
            market.debt_capped_recognized_collateral(margin_position, true, &risk_book)?;
        reconcile_recognition(
            &mut margin_position.recognized_collateral1_for_debt0,
            &mut market.recognition_ledger.debt_bearing_collateral1_for_debt0,
            old_recognized,
            target_recognized,
        )?;
    } else {
        let old_recognized = margin_position.recognized_collateral0_for_debt1;
        let target_recognized =
            market.debt_capped_recognized_collateral(margin_position, false, &risk_book)?;
        reconcile_recognition(
            &mut margin_position.recognized_collateral0_for_debt1,
            &mut market.recognition_ledger.debt_bearing_collateral0_for_debt1,
            old_recognized,
            target_recognized,
        )?;
    }

    market.recognition_ledger.last_recognition_slot = recognition_slot;
    Ok(())
}

fn reconcile_recognition(
    position_recognized: &mut u64,
    ledger_recognized: &mut u64,
    old_recognized: u64,
    target_recognized: u64,
) -> Result<()> {
    match target_recognized.cmp(&old_recognized) {
        std::cmp::Ordering::Greater => {
            let delta = target_recognized
                .checked_sub(old_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *ledger_recognized = ledger_recognized
                .checked_add(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Less => {
            let delta = old_recognized
                .checked_sub(target_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *ledger_recognized = ledger_recognized
                .checked_sub(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Equal => {}
    }

    *position_recognized = target_recognized;
    Ok(())
}

pub(super) fn apply_repay_state(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    repay_asset_is_asset0: bool,
    repay_credit: u64,
) -> Result<()> {
    if repay_asset_is_asset0 {
        let debt_before = margin_position.fixed_debt0(&market.debt_book)?;
        require_gte!(
            debt_before,
            repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let shares_before = margin_position.fixed_debt0_shares;
        let shares_to_burn = if repay_credit as u128 == debt_before {
            shares_before
        } else {
            DebtBook::debt_to_shares(repay_credit, market.debt_book.borrow_index0_nad)?
                .min(shares_before)
        };
        let release_collateral = proportional_release(
            margin_position.recognized_collateral1_for_debt0,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.fixed_debt0_shares = margin_position
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        margin_position.recognized_collateral1_for_debt0 = margin_position
            .recognized_collateral1_for_debt0
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_debt0_shares = market
            .debt_book
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = market
            .recognition_ledger
            .debt_bearing_collateral1_for_debt0
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.side0.reserve_ledger.live_reserve = market
            .side0
            .reserve_ledger
            .live_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market.side0.reserve_ledger.cash_reserve = market
            .side0
            .reserve_ledger
            .cash_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
    } else {
        let debt_before = margin_position.fixed_debt1(&market.debt_book)?;
        require_gte!(
            debt_before,
            repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let shares_before = margin_position.fixed_debt1_shares;
        let shares_to_burn = if repay_credit as u128 == debt_before {
            shares_before
        } else {
            DebtBook::debt_to_shares(repay_credit, market.debt_book.borrow_index1_nad)?
                .min(shares_before)
        };
        let release_collateral = proportional_release(
            margin_position.recognized_collateral0_for_debt1,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.fixed_debt1_shares = margin_position
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        margin_position.recognized_collateral0_for_debt1 = margin_position
            .recognized_collateral0_for_debt1
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.debt_book.fixed_debt1_shares = market
            .debt_book
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.recognition_ledger.debt_bearing_collateral0_for_debt1 = market
            .recognition_ledger
            .debt_bearing_collateral0_for_debt1
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market.side1.reserve_ledger.live_reserve = market
            .side1
            .reserve_ledger
            .live_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market.side1.reserve_ledger.cash_reserve = market
            .side1
            .reserve_ledger
            .cash_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
    }
    market.refresh_market_health()?;
    market.assert_risk_circuit_breakers()
}

fn require_borrow_headroom(debt_side: &MarketSide, borrow_amount: u64) -> Result<()> {
    require_gte!(
        debt_side.reserve_ledger.cash_reserve,
        borrow_amount,
        ErrorCode::InsufficientBorrowHeadroom
    );
    let next_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    require_market_reserve_floor(
        next_reserve,
        debt_side.claim_ledger.protected_claim_supply,
        debt_side.buffer_book.required_buffer,
    )
}

fn proportional_release(recognized: u64, shares_to_burn: u128, shares_before: u128) -> Result<u64> {
    require!(shares_before > 0, ErrorCode::InsufficientDebt);
    if shares_to_burn == shares_before {
        return Ok(recognized);
    }
    let release = (recognized as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(release).map_err(|_| ErrorCode::MarketMathOverflow.into())
}
