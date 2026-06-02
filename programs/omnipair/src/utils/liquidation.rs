use crate::{
    constants::{BPS_DENOMINATOR, LIQUIDATION_INCENTIVE_BPS, LIQUIDATION_PENALTY_BPS},
    errors::ErrorCode,
    utils::{
        gamm_math::CPCurve,
        math::ceil_div,
        token::{transfer_amounts_from_net, TransferAmounts},
    },
};
use anchor_lang::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolventLiquidationPlan {
    pub shares_to_writeoff: u128,
    pub debt_to_writeoff: u64,
    pub collateral_base: u64,
    pub collateral_final: u64,
    pub caller_incentive: u64,
    pub collateral_to_reserves: u64,
    pub collateral_to_reserves_transfer: TransferAmounts,
}

pub fn debt_to_writeoff_for_shares(
    shares_to_writeoff: u128,
    total_debt: u64,
    total_debt_shares: u128,
    user_debt: u64,
) -> Result<u64> {
    if total_debt_shares == 0 || shares_to_writeoff == 0 {
        return Ok(0);
    }
    let debt = shares_to_writeoff
        .checked_mul(total_debt as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(total_debt_shares)
        .ok_or(ErrorCode::DebtMathOverflow)?;
    Ok(user_debt.min(debt.try_into().map_err(|_| ErrorCode::DebtMathOverflow)?))
}

pub fn liquidation_collateral_with_penalty(collateral_base: u64) -> Result<u64> {
    ceil_div(
        (collateral_base as u128)
            .checked_mul((BPS_DENOMINATOR + LIQUIDATION_PENALTY_BPS) as u128)
            .ok_or(ErrorCode::DebtMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::DebtMathOverflow)?
    .try_into()
    .map_err(|_| ErrorCode::DebtMathOverflow.into())
}

pub fn raw_liquidation_incentive(collateral_base: u64) -> Result<u64> {
    Ok((collateral_base as u128)
        .checked_mul(LIQUIDATION_INCENTIVE_BPS as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .try_into()
        .map_err(|_| ErrorCode::DebtMathOverflow)?)
}

pub fn solvent_liquidation_plan_for_shares(
    collateral_mint: &AccountInfo,
    shares_to_writeoff: u128,
    total_debt: u64,
    total_debt_shares: u128,
    user_debt: u64,
    collateral_reserve: u64,
    debt_reserve: u64,
    available_collateral: u64,
) -> Result<Option<SolventLiquidationPlan>> {
    let debt_to_writeoff =
        debt_to_writeoff_for_shares(shares_to_writeoff, total_debt, total_debt_shares, user_debt)?;
    if debt_to_writeoff == 0 || debt_to_writeoff >= debt_reserve {
        return Ok(None);
    }

    let collateral_base =
        CPCurve::calculate_amount_in(collateral_reserve, debt_reserve, debt_to_writeoff)?;
    let collateral_with_penalty = liquidation_collateral_with_penalty(collateral_base)?;
    let caller_incentive = raw_liquidation_incentive(collateral_base)?;
    let reserve_net_target = collateral_with_penalty
        .checked_sub(caller_incentive)
        .ok_or(ErrorCode::DebtMathOverflow)?;
    require_gte!(
        reserve_net_target,
        collateral_base,
        ErrorCode::BrokenInvariant
    );

    let collateral_to_reserves_transfer =
        transfer_amounts_from_net(collateral_mint, reserve_net_target)?;
    let collateral_final = collateral_to_reserves_transfer
        .gross
        .checked_add(caller_incentive)
        .ok_or(ErrorCode::DebtMathOverflow)?;

    if collateral_final > available_collateral {
        return Ok(None);
    }

    Ok(Some(SolventLiquidationPlan {
        shares_to_writeoff,
        debt_to_writeoff,
        collateral_base,
        collateral_final,
        caller_incentive,
        collateral_to_reserves: collateral_to_reserves_transfer.gross,
        collateral_to_reserves_transfer,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_spl::token::Token;

    fn test_mint_account(owner: Pubkey) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0));
        let data = Box::leak(Vec::new().into_boxed_slice());
        let owner = Box::leak(Box::new(owner));
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    #[test]
    fn solvent_plan_targets_reserve_net_not_gross() {
        let mint = test_mint_account(Token::id());
        let plan = solvent_liquidation_plan_for_shares(
            &mint,
            50,
            10_000,
            100,
            10_000,
            1_000_000,
            1_000_000,
            u64::MAX,
        )
        .unwrap()
        .unwrap();

        assert!(plan.collateral_to_reserves_transfer.net >= plan.collateral_base);
        assert_eq!(
            plan.collateral_to_reserves,
            plan.collateral_to_reserves_transfer.gross
        );
        assert_eq!(
            plan.collateral_final,
            plan.collateral_to_reserves_transfer
                .gross
                .checked_add(plan.caller_incentive)
                .unwrap()
        );
    }

    #[test]
    fn solvent_plan_handles_small_unit_close_factor_without_search() {
        let mint = test_mint_account(Token::id());
        let plan = solvent_liquidation_plan_for_shares(
            &mint,
            1_000_000,
            2,
            2_000_000,
            2,
            1_000_000,
            1_000_000,
            u64::MAX,
        )
        .unwrap()
        .unwrap();

        assert_eq!(plan.shares_to_writeoff, 1_000_000);
        assert_eq!(plan.debt_to_writeoff, 1);
    }
}
