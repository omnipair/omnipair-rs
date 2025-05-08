use anchor_lang::prelude::*;
use crate::state::{Pair, UserPosition};
use std::fmt;


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
/// Enum for the different getters that can be emitted
/// This is used to eliminate off-chain calculations / simulation
pub enum PairGetterType {
    EmaPrice0Nad,
    EmaPrice1Nad,
    SpotPrice0Nad,
    SpotPrice1Nad,
}
impl fmt::Display for PairGetterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PairGetterType::EmaPrice0Nad => write!(f, "EmaPrice0Nad"),
            PairGetterType::EmaPrice1Nad => write!(f, "EmaPrice1Nad"),
            PairGetterType::SpotPrice0Nad => write!(f, "SpotPrice0Nad"),
            PairGetterType::SpotPrice1Nad => write!(f, "SpotPrice1Nad"),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum UserPositionGetterType {
    UserToken0BorrowingPower,
    UserToken1BorrowingPower,
    UserToken0EffectiveCollateralFactorBps,
    UserToken1EffectiveCollateralFactorBps,
}
impl fmt::Display for UserPositionGetterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserPositionGetterType::UserToken0BorrowingPower => write!(f, "UserToken0BorrowingPower"),
            UserPositionGetterType::UserToken1BorrowingPower => write!(f, "UserToken1BorrowingPower"),
            UserPositionGetterType::UserToken0EffectiveCollateralFactorBps => write!(f, "UserToken0EffectiveCollateralFactorBps"),
            UserPositionGetterType::UserToken1EffectiveCollateralFactorBps => write!(f, "UserToken1EffectiveCollateralFactorBps"),
        }
    }
}



#[derive(Accounts)]
pub struct EmitPairValue<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
}

#[derive(Accounts)]
pub struct EmitUserPositionValue<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    #[account(mut)]
    pub user_position: Account<'info, UserPosition>,
}

impl EmitPairValue<'_> {
    pub fn handle_emit_value(ctx: Context<Self>, getter: PairGetterType) -> Result<()> {
        let pair = &ctx.accounts.pair;

        let value = match getter {
            PairGetterType::EmaPrice0Nad => pair.ema_price0_nad(),
            PairGetterType::EmaPrice1Nad => pair.ema_price1_nad(),
            PairGetterType::SpotPrice0Nad => pair.spot_price0_nad(),
            PairGetterType::SpotPrice1Nad => pair.spot_price1_nad(),
        };

        msg!("{}: {}", getter, value);

        Ok(())
    }
}

impl EmitUserPositionValue<'_> {
    pub fn handle_emit_value(ctx: Context<Self>, getter: UserPositionGetterType) -> Result<()> {
        let pair = &ctx.accounts.pair;
        let user_position = &ctx.accounts.user_position;

        let value = match getter {
            UserPositionGetterType::UserToken0BorrowingPower => user_position.get_borrowing_power(&pair, &pair.token0),
            UserPositionGetterType::UserToken1BorrowingPower => user_position.get_borrowing_power(&pair, &pair.token1),
            UserPositionGetterType::UserToken0EffectiveCollateralFactorBps => user_position.get_effective_collateral_factor_bps(&pair, &pair.token0),
            UserPositionGetterType::UserToken1EffectiveCollateralFactorBps => user_position.get_effective_collateral_factor_bps(&pair, &pair.token1),
        };

        msg!("{}: {}", getter, value);

        Ok(())
    }
}
