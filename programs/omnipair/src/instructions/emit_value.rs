use anchor_lang::prelude::*;
use crate::state::{FutarchyAuthority, Pair, RateModel, UserLeveragePosition, UserPosition};
use std::fmt;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::gamm_math::{CPCurve, construct_virtual_reserves_at_pessimistic_price};
use crate::utils::math::ceil_div;

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
    pub amount: Option<u64>,
    pub token_mint: Option<Pubkey>,
    /// Used by SimulateLiquidationPrice: the debt amount to simulate borrowing
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
    GetRates,
    GetBorrowLimitAndCfBpsForCollateral,
    Reserves,
    CashReserves,
    SwapQuote,
    /// Quote an isolated leverage open.
    /// Args: amount = margin amount, token_mint = debt token, debt_amount = isolated debt amount.
    /// Returns (collateral_out, closeout_value, equity_bps).
    LeverageOpenQuote,
    /// Simulate liquidation price for a hypothetical new position.
    /// Args: amount = collateral_amount, token_mint = collateral_token, debt_amount = debt to borrow.
    /// Returns NAD-scaled liquidation price of the collateral in debt token units.
    SimulateLiquidationPrice,
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
            PairViewKind::Reserves => write!(f, "Reserves"),
            PairViewKind::CashReserves => write!(f, "CashReserves"),
            PairViewKind::SwapQuote => write!(f, "SwapQuote"),
            PairViewKind::LeverageOpenQuote => write!(f, "LeverageOpenQuote"),
            PairViewKind::SimulateLiquidationPrice => write!(f, "SimulateLiquidationPrice"),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum UserPositionViewKind {
    UserDynamicBorrowLimit,
    UserDynamicCollateralFactorBps,
    UserLiquidationCfBps,
    UserDebtUtilizationBps,
    UserLiquidationPrice,
    UserDebtWithInterest,
    UserIsLiquidatable,
    UserCollateralValueWithImpact,
    UserLiquidationBorrowLimit,
}
impl fmt::Display for UserPositionViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserPositionViewKind::UserDynamicBorrowLimit => write!(f, "UserDynamicBorrowLimit"),
            UserPositionViewKind::UserDynamicCollateralFactorBps => write!(f, "UserDynamicCollateralFactorBps"),
            UserPositionViewKind::UserLiquidationCfBps => write!(f, "UserLiquidationCfBps"),
            UserPositionViewKind::UserDebtUtilizationBps => write!(f, "UserDebtUtilizationBps"),
            UserPositionViewKind::UserLiquidationPrice => write!(f, "UserLiquidationPrice"),
            UserPositionViewKind::UserDebtWithInterest => write!(f, "UserDebtWithInterest"),
            UserPositionViewKind::UserIsLiquidatable => write!(f, "UserIsLiquidatable"),
            UserPositionViewKind::UserCollateralValueWithImpact => write!(f, "UserCollateralValueWithImpact"),
            UserPositionViewKind::UserLiquidationBorrowLimit => write!(f, "UserLiquidationBorrowLimit"),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum LeveragePositionViewKind {
    PositionHealth,
    CloseoutValue,
    CurrentDebt,
    IsLiquidatable,
}

impl fmt::Display for LeveragePositionViewKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LeveragePositionViewKind::PositionHealth => write!(f, "PositionHealth"),
            LeveragePositionViewKind::CloseoutValue => write!(f, "CloseoutValue"),
            LeveragePositionViewKind::CurrentDebt => write!(f, "CurrentDebt"),
            LeveragePositionViewKind::IsLiquidatable => write!(f, "IsLiquidatable"),
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

#[derive(Accounts)]
pub struct ViewLeveragePositionData<'info> {
    pub pair: Account<'info, Pair>,
    #[account(
        constraint = user_leverage_position.pair == pair.key() @ ErrorCode::InvalidPair
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,
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
        pair.update(&ctx.accounts.rate_model, &ctx.accounts.futarchy_authority, pair_key, None)?;

        let empty = || OptionalUint::OptionalU64(None);
        let value: (OptionalUint, OptionalUint, OptionalUint) = match getter {
            PairViewKind::EmaPrice0Nad => (OptionalUint::from_u64(pair.ema_price0_nad()), empty(), empty()),
            PairViewKind::EmaPrice1Nad => (OptionalUint::from_u64(pair.ema_price1_nad()), empty(), empty()),
            PairViewKind::SpotPrice0Nad => (OptionalUint::from_u64(pair.spot_price0_nad()), empty(), empty()),
            PairViewKind::SpotPrice1Nad => (OptionalUint::from_u64(pair.spot_price1_nad()), empty(), empty()),
            PairViewKind::K => (OptionalUint::from_u128(pair.k()), empty(), empty()),
            PairViewKind::GetRates => {
                let (rate0, rate1) = pair.get_rates(&ctx.accounts.rate_model).unwrap();
                (OptionalUint::from_u64(rate0), OptionalUint::from_u64(rate1), empty())
            },
            PairViewKind::GetBorrowLimitAndCfBpsForCollateral => {
                let collateral_amount = args.amount.ok_or(ErrorCode::ArgumentMissing)?;
                let collateral_token = args.token_mint.ok_or(ErrorCode::ArgumentMissing)?;
                let (borrow_limit, max_cf_bps, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount).unwrap();
                (
                    OptionalUint::from_u64(borrow_limit),
                    OptionalUint::from_u16(max_cf_bps),
                    OptionalUint::from_u16(liquidation_cf_bps),
                )
            },
            PairViewKind::Reserves => (
                OptionalUint::from_u64(pair.reserve0),
                OptionalUint::from_u64(pair.reserve1),
                empty(),
            ),
            PairViewKind::CashReserves => (
                OptionalUint::from_u64(pair.cash_reserve0),
                OptionalUint::from_u64(pair.cash_reserve1),
                empty(),
            ),
            PairViewKind::SwapQuote => {
                // Preview swap: given amount_in of collateral_token, returns (amount_out, swap_fee)
                let amount_in = args.amount.ok_or(ErrorCode::ArgumentMissing)?;
                let token_in = args.token_mint.ok_or(ErrorCode::ArgumentMissing)?;
                let is_token0_in = token_in == pair.token0;

                let swap_fee = ceil_div(
                    (amount_in as u128)
                        .checked_mul(pair.swap_fee_bps as u128)
                        .ok_or(ErrorCode::FeeMathOverflow)?,
                    BPS_DENOMINATOR as u128,
                ).ok_or(ErrorCode::FeeMathOverflow)? as u64;

                let amount_in_after_fee = amount_in.checked_sub(swap_fee).ok_or(ErrorCode::FeeMathOverflow)?;

                let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
                let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };

                let amount_out = CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_fee)?;

                (OptionalUint::from_u64(amount_out), OptionalUint::from_u64(swap_fee), empty())
            },
            PairViewKind::LeverageOpenQuote => {
                let margin_amount = args.amount.ok_or(ErrorCode::ArgumentMissing)?;
                let debt_amount = args.debt_amount.ok_or(ErrorCode::ArgumentMissing)?;
                let debt_token = args.token_mint.ok_or(ErrorCode::ArgumentMissing)?;
                let notional = margin_amount
                    .checked_add(debt_amount)
                    .ok_or(ErrorCode::Overflow)?;
                let is_debt_token0 = debt_token == pair.token0;
                require!(
                    debt_token == pair.token0 || debt_token == pair.token1,
                    ErrorCode::InvalidMint
                );

                let reserve_in = if is_debt_token0 { pair.reserve0 } else { pair.reserve1 };
                let reserve_out = if is_debt_token0 { pair.reserve1 } else { pair.reserve0 };
                let swap_fee = ceil_div(
                    (notional as u128)
                        .checked_mul(pair.swap_fee_bps as u128)
                        .ok_or(ErrorCode::FeeMathOverflow)?,
                    BPS_DENOMINATOR as u128,
                ).ok_or(ErrorCode::FeeMathOverflow)? as u64;
                let protocol_fee = ceil_div(
                    (swap_fee as u128)
                        .checked_mul(ctx.accounts.futarchy_authority.revenue_share.swap_bps as u128)
                        .ok_or(ErrorCode::FeeMathOverflow)?,
                    BPS_DENOMINATOR as u128,
                ).ok_or(ErrorCode::FeeMathOverflow)? as u64;
                let collateral_out = leverage_quote_swap(notional, reserve_in, reserve_out, pair.swap_fee_bps)?;
                let post_reserve_in = reserve_in
                    .checked_add(notional.checked_sub(protocol_fee).ok_or(ErrorCode::FeeMathOverflow)?)
                    .ok_or(ErrorCode::Overflow)?;
                let post_reserve_out = reserve_out
                    .checked_sub(collateral_out)
                    .ok_or(ErrorCode::Overflow)?;
                let closeout_value = leverage_quote_swap(
                    collateral_out,
                    post_reserve_out,
                    post_reserve_in,
                    pair.swap_fee_bps,
                )?;
                let equity = closeout_value.saturating_sub(debt_amount);
                let equity_bps = match closeout_value {
                    0 => 0,
                    _ => (equity as u128)
                        .checked_mul(BPS_DENOMINATOR as u128)
                        .ok_or(ErrorCode::Overflow)?
                        .checked_div(closeout_value as u128)
                        .ok_or(ErrorCode::Overflow)? as u64,
                };
                (
                    OptionalUint::from_u64(collateral_out),
                    OptionalUint::from_u64(closeout_value),
                    OptionalUint::from_u64(equity_bps),
                )
            },
            PairViewKind::SimulateLiquidationPrice => {
                // Simulate liquidation price for a hypothetical new position
                // amount = collateral_amount, token_mint = collateral_token, debt_amount = debt to borrow
                let collateral_amount = args.amount.ok_or(ErrorCode::ArgumentMissing)?;
                let collateral_token = args.token_mint.ok_or(ErrorCode::ArgumentMissing)?;
                let debt_amount = args.debt_amount.ok_or(ErrorCode::ArgumentMissing)?;

                if debt_amount == 0 {
                    return Ok(()); // No debt = no liquidation price
                }

                // Compute the liquidation CF that would be locked in at borrow time
                let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(
                    &pair, &collateral_token, collateral_amount
                )?;

                let cf_bps = liquidation_cf_bps as u128;
                if collateral_amount == 0 || cf_bps == 0 {
                    // Immediately unsafe
                    let value = (OptionalUint::from_u64(u64::MAX), empty(), empty());
                    msg!("{}: {:?}", getter, value);
                    return Ok(());
                }

                // Determine decimal adjustments
                let is_collateral_token0 = collateral_token == pair.token0;
                let (collateral_decimals, debt_decimals) = if is_collateral_token0 {
                    (pair.token0_decimals as i32, pair.token1_decimals as i32)
                } else {
                    (pair.token1_decimals as i32, pair.token0_decimals as i32)
                };

                // P* (NAD) = ceil( debt * 10^{collateral_decimals} * NAD * BPS / (collateral * 10^{debt_decimals} * CF_BPS) )
                let dec_diff = collateral_decimals - debt_decimals;
                let (num_dec_mul, den_dec_mul): (u128, u128) = if dec_diff >= 0 {
                    (10u128.pow(dec_diff as u32), 1)
                } else {
                    (1, 10u128.pow((-dec_diff) as u32))
                };

                let num = (debt_amount as u128)
                    .saturating_mul(num_dec_mul)
                    .saturating_mul(NAD as u128)
                    .saturating_mul(BPS_DENOMINATOR as u128);

                let den = (collateral_amount as u128)
                    .saturating_mul(den_dec_mul)
                    .saturating_mul(cf_bps);

                let p_star_nad = if den == 0 {
                    u64::MAX
                } else {
                    num.saturating_add(den.saturating_sub(1))
                        .checked_div(den)
                        .unwrap_or(u128::MAX)
                        .min(u64::MAX as u128) as u64
                };

                (OptionalUint::from_u64(p_star_nad), OptionalUint::from_u16(liquidation_cf_bps), empty())
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
        pair.update(&ctx.accounts.rate_model, &ctx.accounts.futarchy_authority, pair_key, None)?;

        let empty = || OptionalUint::OptionalU64(None);
        let value: (OptionalUint, OptionalUint, OptionalUint) = match getter {
            UserPositionViewKind::UserDynamicBorrowLimit => {
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
                    empty(),
                )
            },
            UserPositionViewKind::UserDynamicCollateralFactorBps => {
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
                    OptionalUint::from_u16(token1_cf_bps),
                    empty(),
                )
            },
            UserPositionViewKind::UserLiquidationCfBps => (
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u16(user_position.get_liquidation_cf_bps(&pair, &pair.token1).unwrap()),
                empty(),
            ),
            UserPositionViewKind::UserDebtUtilizationBps => (
                OptionalUint::from_u64(user_position.get_debt_utilization_bps(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u64(user_position.get_debt_utilization_bps(&pair, &pair.token1).unwrap()),
                empty(),
            ),
            UserPositionViewKind::UserLiquidationPrice => (
                OptionalUint::from_u64(user_position.get_liquidation_price(&pair, &pair.token0).unwrap()),
                OptionalUint::from_u64(user_position.get_liquidation_price(&pair, &pair.token1).unwrap()),
                empty(),
            ),
            UserPositionViewKind::UserDebtWithInterest => (
                OptionalUint::from_u64(user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares).unwrap()),
                OptionalUint::from_u64(user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares).unwrap()),
                empty(),
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
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve0, pair.reserve1, pair.ema_price0_nad(), pair.directional_ema_price0_nad()
                        ).unwrap_or((pair.reserve0, pair.reserve1));
                        
                        let collateral_value = CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0);
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
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve1, pair.reserve0, pair.ema_price1_nad(), pair.directional_ema_price1_nad()
                        ).unwrap_or((pair.reserve1, pair.reserve0));
                        
                        let collateral_value = CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0);
                        let borrow_limit = (collateral_value as u128)
                            .saturating_mul(liquidation_cf as u128)
                            .checked_div(BPS_DENOMINATOR as u128).unwrap_or(0);
                        
                        if (user_debt as u128) >= borrow_limit { 1 } else { 0 }
                    }
                };
                
                (OptionalUint::from_u64(is_liquidatable_0), OptionalUint::from_u64(is_liquidatable_1), empty())
            },
            UserPositionViewKind::UserCollateralValueWithImpact => {
                // Collateral value in debt-token units with price impact (same math as liquidation)
                // Token0 as collateral (backing token1 debt)
                let value0 = {
                    let user_collateral = user_position.collateral0;
                    if user_collateral == 0 {
                        0u64
                    } else {
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve0, pair.reserve1, pair.ema_price0_nad(), pair.ema_price0_nad()
                        ).unwrap_or((pair.reserve0, pair.reserve1));
                        CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0)
                    }
                };
                // Token1 as collateral (backing token0 debt)
                let value1 = {
                    let user_collateral = user_position.collateral1;
                    if user_collateral == 0 {
                        0u64
                    } else {
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve1, pair.reserve0, pair.ema_price1_nad(), pair.ema_price1_nad()
                        ).unwrap_or((pair.reserve1, pair.reserve0));
                        CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0)
                    }
                };
                (OptionalUint::from_u64(value0), OptionalUint::from_u64(value1), empty())
            },
            UserPositionViewKind::UserLiquidationBorrowLimit => {
                // Liquidation borrow limit = collateral_value_with_impact * liquidation_cf / BPS
                // Position is liquidatable when debt >= this value
                // Token0 as collateral (backing token1 debt)
                let limit0 = {
                    let user_collateral = user_position.collateral0;
                    let liquidation_cf = user_position.get_liquidation_cf_bps(&pair, &pair.token1).unwrap_or(0);
                    if user_collateral == 0 || liquidation_cf == 0 {
                        0u64
                    } else {
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve0, pair.reserve1, pair.ema_price0_nad(), pair.ema_price0_nad()
                        ).unwrap_or((pair.reserve0, pair.reserve1));
                        let collateral_value = CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0);
                        (collateral_value as u128)
                            .saturating_mul(liquidation_cf as u128)
                            .checked_div(BPS_DENOMINATOR as u128)
                            .unwrap_or(0) as u64
                    }
                };
                // Token1 as collateral (backing token0 debt)
                let limit1 = {
                    let user_collateral = user_position.collateral1;
                    let liquidation_cf = user_position.get_liquidation_cf_bps(&pair, &pair.token0).unwrap_or(0);
                    if user_collateral == 0 || liquidation_cf == 0 {
                        0u64
                    } else {
                        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
                            pair.reserve1, pair.reserve0, pair.ema_price1_nad(), pair.ema_price1_nad()
                        ).unwrap_or((pair.reserve1, pair.reserve0));
                        let collateral_value = CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral).unwrap_or(0);
                        (collateral_value as u128)
                            .saturating_mul(liquidation_cf as u128)
                            .checked_div(BPS_DENOMINATOR as u128)
                            .unwrap_or(0) as u64
                    }
                };
                (OptionalUint::from_u64(limit0), OptionalUint::from_u64(limit1), empty())
            },
        };

        msg!("{}: {:?}", getter, value);

        Ok(())
    }
}

impl ViewLeveragePositionData<'_> {
    pub fn handle_view_data(ctx: Context<Self>, getter: LeveragePositionViewKind) -> Result<()> {
        let pair_key = ctx.accounts.pair.key();
        let mut pair = ctx.accounts.pair.clone().into_inner();
        let user_leverage_position = &ctx.accounts.user_leverage_position;

        pair.update(&ctx.accounts.rate_model, &ctx.accounts.futarchy_authority, pair_key, None)?;

        let empty = || OptionalUint::OptionalU64(None);
        let debt = user_leverage_position.calculate_debt(&pair)?;
        let is_collateral_token0 = !user_leverage_position.is_debt_token0;
        let reserve_in = if is_collateral_token0 { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_collateral_token0 { pair.reserve1 } else { pair.reserve0 };
        let closeout_value = match user_leverage_position.collateral_amount {
            0 => 0,
            collateral_amount => leverage_quote_swap(
                collateral_amount,
                reserve_in,
                reserve_out,
                pair.swap_fee_bps,
            )?,
        };
        let equity = closeout_value.saturating_sub(debt);
        let equity_bps = match closeout_value {
            0 => 0,
            _ => (equity as u128)
                .checked_mul(BPS_DENOMINATOR as u128)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(closeout_value as u128)
                .ok_or(ErrorCode::Overflow)? as u64,
        };
        let is_liquidatable = if closeout_value <= debt
            || equity_bps <= LEVERAGE_MAINTENANCE_BUFFER_BPS as u64
        {
            1
        } else {
            0
        };

        let value: (OptionalUint, OptionalUint, OptionalUint) = match getter {
            LeveragePositionViewKind::PositionHealth => (
                OptionalUint::from_u64(debt),
                OptionalUint::from_u64(closeout_value),
                OptionalUint::from_u64(equity_bps),
            ),
            LeveragePositionViewKind::CloseoutValue => (
                OptionalUint::from_u64(closeout_value),
                empty(),
                empty(),
            ),
            LeveragePositionViewKind::CurrentDebt => (
                OptionalUint::from_u64(debt),
                empty(),
                empty(),
            ),
            LeveragePositionViewKind::IsLiquidatable => (
                OptionalUint::from_u64(is_liquidatable),
                OptionalUint::from_u64(debt),
                OptionalUint::from_u64(closeout_value),
            ),
        };

        msg!("{}: {:?}", getter, value);

        Ok(())
    }
}

fn leverage_quote_swap(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    swap_fee_bps: u16,
) -> Result<u64> {
    require!(amount_in > 0, ErrorCode::AmountZero);
    require!(reserve_in > 0 && reserve_out > 0, ErrorCode::InsufficientLiquidity);

    let swap_fee = ceil_div(
        (amount_in as u128)
            .checked_mul(swap_fee_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;
    let amount_in_after_fee = amount_in
        .checked_sub(swap_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;

    CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_fee)
}
