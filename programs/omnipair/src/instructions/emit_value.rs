use anchor_lang::prelude::*;
use crate::state::{Pair, UserPosition, RateModel, FutarchyAuthority};
use std::fmt;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::gamm_math::{CPCurve, construct_virtual_reserves_at_pessimistic_price};

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
    UserIsLiquidatable,
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
            UserPositionViewKind::UserIsLiquidatable => write!(f, "UserIsLiquidatable"),
        }
    }
}



#[derive(Accounts)]
pub struct ViewPairData<'info> {
    pub pair: Account<'info, Pair>,
    #[account(
        address = pair.rate_model @ ErrorCode::InvalidRateModel
    )]
    pub rate_model: Account<'info, RateModel>,
    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,
}

#[derive(Accounts)]
pub struct ViewUserPositionData<'info> {
    pub pair: Account<'info, Pair>,
    #[account(
        constraint = user_position.pair == pair.key() @ ErrorCode::InvalidPair
    )]
    pub user_position: Account<'info, UserPosition>,
    #[account(
        address = pair.rate_model @ ErrorCode::InvalidRateModel
    )]
    pub rate_model: Account<'info, RateModel>,
    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,
}

impl ViewPairData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: PairViewKind, args: EmitValueArgs) -> Result<()> {
        // Create a copy of the pair state to perform simulated update without modifying the actual account
        let pair_key = ctx.accounts.pair.key();
        let mut pair = ctx.accounts.pair.clone().into_inner();
        
        // update pair to get updated rates, interest, debt, etc.
        pair.update(&ctx.accounts.rate_model, &ctx.accounts.futarchy_authority, pair_key)?;

        let value: (OptionalUint, OptionalUint) = match getter {
            PairViewKind::EmaPrice0Nad => (OptionalUint::from_u64(pair.ema_price0_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::EmaPrice1Nad => (OptionalUint::from_u64(pair.ema_price1_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::SpotPrice0Nad => (OptionalUint::from_u64(pair.spot_price0_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::SpotPrice1Nad => (OptionalUint::from_u64(pair.spot_price1_nad()), OptionalUint::OptionalU64(None)),
            PairViewKind::K => (OptionalUint::from_u128(pair.k()), OptionalUint::OptionalU128(None)),
            PairViewKind::GetRates => {
                let (rate0, rate1) = pair.get_rates(&ctx.accounts.rate_model).unwrap();
                (OptionalUint::from_u64(rate0), OptionalUint::from_u64(rate1))
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
        // Create a copy of the pair state to perform simulated update without modifying the actual account
        let pair_key = ctx.accounts.pair.key();
        let mut pair = ctx.accounts.pair.clone().into_inner();
        let user_position = &ctx.accounts.user_position;

        // update pair to get updated rates, interest, debt, etc.
        pair.update(&ctx.accounts.rate_model, &ctx.accounts.futarchy_authority, pair_key)?;

        let value: (OptionalUint, OptionalUint) = match getter {
            UserPositionViewKind::UserBorrowingPower => {
                let collateral_token0 = pair.get_collateral_token(&pair.token0);
                let collateral_amount0 = match collateral_token0 == pair.token0 {
                    true => user_position.collateral0,
                    false => user_position.collateral1,
                };
                let borrow_limit0 = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token0, collateral_amount0).unwrap().0;
                
                let collateral_token1 = pair.get_collateral_token(&pair.token1);
                let collateral_amount1 = match collateral_token1 == pair.token0 {
                    true => user_position.collateral0,
                    false => user_position.collateral1,
                };
                let borrow_limit1 = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token1, collateral_amount1).unwrap().0;
                
                (
                    OptionalUint::from_u64(borrow_limit0),
                    OptionalUint::from_u64(borrow_limit1),
                )
            },
            UserPositionViewKind::UserAppliedCollateralFactorBps => {
                let collateral_token0 = pair.get_collateral_token(&pair.token0);
                let collateral_amount0 = match collateral_token0 == pair.token0 {
                    true => user_position.collateral0,
                    false => user_position.collateral1,
                };
                let token0_cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token0, collateral_amount0).unwrap().1;
                
                let collateral_token1 = pair.get_collateral_token(&pair.token1);
                let collateral_amount1 = match collateral_token1 == pair.token0 {
                    true => user_position.collateral0,
                    false => user_position.collateral1,
                };
                let token1_cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token1, collateral_amount1).unwrap().1;
                
                (
                    OptionalUint::from_u16(token0_cf_bps),
                    OptionalUint::from_u16(token1_cf_bps)
                )
            },
            UserPositionViewKind::UserLiquidationCollateralFactorBps => (
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token1).unwrap())
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
            UserPositionViewKind::UserIsLiquidatable => {
                // Check liquidatability for both directions using price impact
                // Returns (is_liquidatable_with_token0_collateral, is_liquidatable_with_token1_collateral)
                
                // Token0 as collateral, Token1 as debt
                let is_liquidatable_0 = {
                    let user_debt = user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares).unwrap_or(0);
                    let user_collateral = user_position.collateral0;
                    let liquidation_cf = user_position.get_liquidation_cf_bps(&pair, &pair.token1).unwrap_or(0);
                    
                    if user_debt == 0 || user_collateral == 0 {
                        0u64
                    } else {
                        let (x_virt, y_virt) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve0, pair.reserve1, pair.ema_price0_nad(), pair.ema_price0_nad()
                        ).unwrap_or((pair.reserve0, pair.reserve1));
                        
                        let collateral_value = CPCurve::calculate_amount_out(y_virt, x_virt, user_collateral).unwrap_or(0);
                        let borrow_limit = (collateral_value as u128)
                            .saturating_mul(liquidation_cf as u128)
                            .checked_div(BPS_DENOMINATOR as u128).unwrap_or(0);
                        
                        if (user_debt as u128) >= borrow_limit { 1 } else { 0 }
                    }
                };
                
                // Token1 as collateral, Token0 as debt
                let is_liquidatable_1 = {
                    let user_debt = user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares).unwrap_or(0);
                    let user_collateral = user_position.collateral1;
                    let liquidation_cf = user_position.get_liquidation_cf_bps(&pair, &pair.token0).unwrap_or(0);
                    
                    if user_debt == 0 || user_collateral == 0 {
                        0u64
                    } else {
                        let (x_virt, y_virt) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve1, pair.reserve0, pair.ema_price1_nad(), pair.ema_price1_nad()
                        ).unwrap_or((pair.reserve1, pair.reserve0));
                        
                        let collateral_value = CPCurve::calculate_amount_out(y_virt, x_virt, user_collateral).unwrap_or(0);
                        let borrow_limit = (collateral_value as u128)
                            .saturating_mul(liquidation_cf as u128)
                            .checked_div(BPS_DENOMINATOR as u128).unwrap_or(0);
                        
                        if (user_debt as u128) >= borrow_limit { 1 } else { 0 }
                    }
                };
                
                (OptionalUint::from_u64(is_liquidatable_0), OptionalUint::from_u64(is_liquidatable_1))
            },
        };

        msg!("{}: {:?}", getter, value);

        Ok(())
    }
}
