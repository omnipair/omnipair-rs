use anchor_lang::prelude::*;
use crate::state::{Pair, UserPosition};
use std::fmt;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct EmitValueArgs {
    pub debt_amount: Option<u64>,
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
/// Enum for the different getters that can be emitted
/// This is used to eliminate off-chain calculations / simulation
pub enum PairViewKind {
    EmaPrice0Nad,
    EmaPrice1Nad,
    SpotPrice0Nad,
    SpotPrice1Nad,
    K,
}
impl fmt::Display for PairViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PairViewKind::EmaPrice0Nad => write!(f, "EmaPrice0Nad"),
            PairViewKind::EmaPrice1Nad => write!(f, "EmaPrice1Nad"),
            PairViewKind::SpotPrice0Nad => write!(f, "SpotPrice0Nad"),
            PairViewKind::SpotPrice1Nad => write!(f, "SpotPrice1Nad"),
            PairViewKind::K => write!(f, "K"),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum UserPositionViewKind {
    UserToken0BorrowingPower,
    UserToken1BorrowingPower,
    UserToken0EffectiveCollateralFactorBps,
    UserToken1EffectiveCollateralFactorBps,
    UserToken0LiquidationCollateralFactorBps,
    UserToken1LiquidationCollateralFactorBps,
    UserToken0MinCollateralForDebt,
    UserToken1MinCollateralForDebt,
    UserToken0DebtUtilizationBps,
    UserToken1DebtUtilizationBps,
}
impl fmt::Display for UserPositionViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserPositionViewKind::UserToken0BorrowingPower => write!(f, "UserToken0BorrowingPower"),
            UserPositionViewKind::UserToken1BorrowingPower => write!(f, "UserToken1BorrowingPower"),
            UserPositionViewKind::UserToken0EffectiveCollateralFactorBps => write!(f, "UserToken0EffectiveCollateralFactorBps"),
            UserPositionViewKind::UserToken1EffectiveCollateralFactorBps => write!(f, "UserToken1EffectiveCollateralFactorBps"),
            UserPositionViewKind::UserToken0LiquidationCollateralFactorBps => write!(f, "UserToken0LiquidationCollateralFactorBps"),
            UserPositionViewKind::UserToken1LiquidationCollateralFactorBps => write!(f, "UserToken1LiquidationCollateralFactorBps"),
            UserPositionViewKind::UserToken0MinCollateralForDebt => write!(f, "UserToken0MinCollateralForDebt"),
            UserPositionViewKind::UserToken1MinCollateralForDebt => write!(f, "UserToken1MinCollateralForDebt"),
            UserPositionViewKind::UserToken0DebtUtilizationBps => write!(f, "UserToken0DebtUtilizationBps"),
            UserPositionViewKind::UserToken1DebtUtilizationBps => write!(f, "UserToken1DebtUtilizationBps"),
        }
    }
}



#[derive(Accounts)]
pub struct ViewPairData<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
}

#[derive(Accounts)]
pub struct ViewUserPositionData<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    #[account(mut)]
    pub user_position: Account<'info, UserPosition>,
}

impl ViewPairData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: PairViewKind) -> Result<()> {
        let pair = &ctx.accounts.pair;

        let value = match getter {
            PairViewKind::EmaPrice0Nad => pair.ema_price0_nad(),
            PairViewKind::EmaPrice1Nad => pair.ema_price1_nad(),
            PairViewKind::SpotPrice0Nad => pair.spot_price0_nad(),
            PairViewKind::SpotPrice1Nad => pair.spot_price1_nad(),
            PairViewKind::K => pair.k() as u64,
        };

        msg!("{}: {}", getter, value);

        Ok(())
    }
}

impl ViewUserPositionData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: UserPositionViewKind, args: EmitValueArgs) -> Result<()> {
        let pair = &ctx.accounts.pair;
        let user_position = &ctx.accounts.user_position;

        let value = match getter {
            UserPositionViewKind::UserToken0BorrowingPower => user_position.get_borrow_limit(&pair, &pair.token0),
            UserPositionViewKind::UserToken1BorrowingPower => user_position.get_borrow_limit(&pair, &pair.token1),
            UserPositionViewKind::UserToken0EffectiveCollateralFactorBps => user_position.get_pessimistic_collateral_factor_bps(&pair, &pair.token0) as u64,
            UserPositionViewKind::UserToken1EffectiveCollateralFactorBps => user_position.get_pessimistic_collateral_factor_bps(&pair, &pair.token1) as u64,
            UserPositionViewKind::UserToken0LiquidationCollateralFactorBps => user_position.get_liquidation_cf_bps(&pair, &pair.token0) as u64,
            UserPositionViewKind::UserToken1LiquidationCollateralFactorBps => user_position.get_liquidation_cf_bps(&pair, &pair.token1) as u64,
            UserPositionViewKind::UserToken0MinCollateralForDebt => {
                let debt_amount = args.debt_amount.ok_or(ErrorCode::ArgumentMissing)?;
                user_position.get_min_collateral_and_cf_bps_for_debt(&pair, debt_amount).unwrap().0
            },
            UserPositionViewKind::UserToken1MinCollateralForDebt => {
                let debt_amount = args.debt_amount.ok_or(ErrorCode::ArgumentMissing)?;
                user_position.get_min_collateral_and_cf_bps_for_debt(&pair, debt_amount).unwrap().0
            },
            UserPositionViewKind::UserToken0DebtUtilizationBps => user_position.get_debt_utilization_bps(&pair, &pair.token0).unwrap(),
            UserPositionViewKind::UserToken1DebtUtilizationBps => user_position.get_debt_utilization_bps(&pair, &pair.token1).unwrap(),
        };

        msg!("{}: {}", getter, value);

        Ok(())
    }
}
