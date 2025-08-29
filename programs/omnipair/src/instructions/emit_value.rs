use anchor_lang::prelude::*;
use crate::state::{Pair, UserPosition, RateModel};
use std::fmt;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum OptionalUint {
    U64(u64),
    U128(u128),
    U16(u16),
    OptionalU64(Option<u64>),
    OptionalU128(Option<u128>),
    OptionalU16(Option<u16>),
}

impl fmt::Display for OptionalUint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OptionalUint::U64(val) => write!(f, "{}", val),
            OptionalUint::U128(val) => write!(f, "{}", val),
            OptionalUint::U16(val) => write!(f, "{}", val),
            OptionalUint::OptionalU64(val) => match val {
                Some(v) => write!(f, "Some({})", v),
                None => write!(f, "None"),
            },
            OptionalUint::OptionalU128(val) => match val {
                Some(v) => write!(f, "Some({})", v),
                None => write!(f, "None"),
            },
            OptionalUint::OptionalU16(val) => match val {
                Some(v) => write!(f, "Some({})", v),
                None => write!(f, "None"),
            },
        }
    }
}

impl OptionalUint {
    pub fn from_u64(val: u64) -> Self { OptionalUint::U64(val) }
    pub fn from_u128(val: u128) -> Self { OptionalUint::U128(val) }
    pub fn from_u16(val: u16) -> Self { OptionalUint::U16(val) }
    pub fn from_optional_u64(val: Option<u64>) -> Self { OptionalUint::OptionalU64(val) }
    pub fn from_optional_u128(val: Option<u128>) -> Self { OptionalUint::OptionalU128(val) }
    pub fn from_optional_u16(val: Option<u16>) -> Self { OptionalUint::OptionalU16(val) }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct EmitValueArgs {
    pub debt_amount: Option<u64>,
    pub collateral_amount: Option<u64>,
    pub collateral_token: Option<Pubkey>,
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
    GetRates,
    GetMinCollateralForDebt,
    GetBorrowLimitAndCfBpsForCollateral,

}
impl fmt::Display for PairViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PairViewKind::EmaPrice0Nad => write!(f, "EmaPrice0Nad"),
            PairViewKind::EmaPrice1Nad => write!(f, "EmaPrice1Nad"),
            PairViewKind::SpotPrice0Nad => write!(f, "SpotPrice0Nad"),
            PairViewKind::SpotPrice1Nad => write!(f, "SpotPrice1Nad"),
            PairViewKind::K => write!(f, "K"),
            PairViewKind::GetRates => write!(f, "GetRates"),
            PairViewKind::GetMinCollateralForDebt => write!(f, "GetMinCollateralForDebt"),
            PairViewKind::GetBorrowLimitAndCfBpsForCollateral => write!(f, "GetBorrowLimitAndCfBpsForCollateral"),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum UserPositionViewKind {
    UserBorrowingPower,
    UserAppliedCollateralFactorBps,
    UserLiquidationCollateralFactorBps,
    UserDebtUtilizationBps,
    UserLiquidationPrice,
    UserDebtWithInterest,
}
impl fmt::Display for UserPositionViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserPositionViewKind::UserBorrowingPower => write!(f, "UserBorrowingPower"),
            UserPositionViewKind::UserAppliedCollateralFactorBps => write!(f, "UserAppliedCollateralFactorBps"),
            UserPositionViewKind::UserLiquidationCollateralFactorBps => write!(f, "UserLiquidationCollateralFactorBps"),
            UserPositionViewKind::UserDebtUtilizationBps => write!(f, "UserDebtUtilizationBps"),
            UserPositionViewKind::UserLiquidationPrice => write!(f, "UserLiquidationPrice"),
            UserPositionViewKind::UserDebtWithInterest => write!(f, "UserDebtWithInterest"),
        }
    }
}



#[derive(Accounts)]
pub struct ViewPairData<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    pub rate_model: Account<'info, RateModel>,
}

#[derive(Accounts)]
pub struct ViewUserPositionData<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    #[account(mut)]
    pub user_position: Account<'info, UserPosition>,
    pub rate_model: Account<'info, RateModel>,
}

impl ViewPairData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: PairViewKind, args: EmitValueArgs) -> Result<()> {
        let pair = &mut ctx.accounts.pair;

        // update pair to get updated rates, interest, debt, etc.
        pair.update(&ctx.accounts.rate_model)?;

        let value: (OptionalUint, OptionalUint) = match getter {
            PairViewKind::EmaPrice0Nad => (OptionalUint::from_u64(pair.ema_price0_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::EmaPrice1Nad => (OptionalUint::from_u64(pair.ema_price1_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::SpotPrice0Nad => (OptionalUint::from_u64(pair.spot_price0_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::SpotPrice1Nad => (OptionalUint::from_u64(pair.spot_price1_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::K => (OptionalUint::from_u64(pair.k() as u64), OptionalUint::OptionalU64(None)),
            PairViewKind::GetRates => {
                let (rate0, rate1) = pair.get_rates(&ctx.accounts.rate_model).unwrap();
                (OptionalUint::from_u64(rate0), OptionalUint::from_u64(rate1))
            },
            PairViewKind::GetMinCollateralForDebt => {
                let debt_amount = args.debt_amount.ok_or(ErrorCode::ArgumentMissing)?;
                (
                    OptionalUint::from_u64(pair.get_min_collateral_and_cf_bps_for_debt(&pair, &pair.token0, debt_amount).unwrap().0),
                    OptionalUint::from_u16(pair.get_min_collateral_and_cf_bps_for_debt(&pair, &pair.token1, debt_amount).unwrap().1)
                )
            },
            PairViewKind::GetBorrowLimitAndCfBpsForCollateral => {
                let collateral_amount = args.collateral_amount.ok_or(ErrorCode::ArgumentMissing)?;
                let collateral_token = args.collateral_token.ok_or(ErrorCode::ArgumentMissing)?;
                (
                    OptionalUint::from_u64(pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount).unwrap().0),
                    OptionalUint::from_u16(pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount).unwrap().1)
                )
            },
        };

        msg!("{}: {:?}", getter, value);

        Ok(())
    }
}

impl ViewUserPositionData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: UserPositionViewKind) -> Result<()> {
        let pair = &mut ctx.accounts.pair;
        let user_position = &ctx.accounts.user_position;

        // update pair to get updated rates, interest, debt, etc.
        pair.update(&ctx.accounts.rate_model)?;

        let value: (OptionalUint, OptionalUint) = match getter {
            UserPositionViewKind::UserBorrowingPower => (
                OptionalUint::from_u64(user_position.get_user_borrow_limit(&pair, &pair.token0)),
                OptionalUint::from_u64(user_position.get_user_borrow_limit(&pair, &pair.token1)),
            ),
            UserPositionViewKind::UserAppliedCollateralFactorBps => (
                OptionalUint::from_u16(user_position.get_user_pessimistic_collateral_factor_bps(&pair, &pair.token0)),
                OptionalUint::from_u16(user_position.get_user_pessimistic_collateral_factor_bps(&pair, &pair.token1))
            ),
            UserPositionViewKind::UserLiquidationCollateralFactorBps => (
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token0)),
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token1))
            ),
            UserPositionViewKind::UserDebtUtilizationBps => (
                OptionalUint::from_u64(user_position.get_debt_utilization_bps(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u64(user_position.get_debt_utilization_bps(&pair, &pair.token1).unwrap())
            ),
            UserPositionViewKind::UserLiquidationPrice => (
                OptionalUint::from_u64(user_position.get_liquidation_price(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u64(user_position.get_liquidation_price(&pair, &pair.token1).unwrap())
            ),
            UserPositionViewKind::UserDebtWithInterest => (
                OptionalUint::from_u64(user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares).unwrap()),
                OptionalUint::from_u64(user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares).unwrap())
            ),
        };

        msg!("{}: {:?}", getter, value);

        Ok(())
    }
}
