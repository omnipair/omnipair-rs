use anchor_lang::prelude::*;
use crate::state::Pair;
use std::fmt;

/// Enum for the different getters that can be emitted
/// This is used to eliminate off-chain calculations
/// and to simulate the on-chain getters
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum GetterType {
    EmaPrice0Nad,
    EmaPrice1Nad,
    SpotPrice0Nad,
    SpotPrice1Nad,
}
impl fmt::Display for GetterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GetterType::EmaPrice0Nad => write!(f, "EmaPrice0Nad"),
            GetterType::EmaPrice1Nad => write!(f, "EmaPrice1Nad"),
            GetterType::SpotPrice0Nad => write!(f, "SpotPrice0Nad"),
            GetterType::SpotPrice1Nad => write!(f, "SpotPrice1Nad"),
        }
    }
}

#[derive(Accounts)]
pub struct EmitValue<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
}

impl EmitValue<'_> {
    pub fn handle_emit_value(ctx: Context<Self>, getter: GetterType) -> Result<()> {
        let pair = &ctx.accounts.pair;
        let value = match getter {
            GetterType::EmaPrice0Nad => pair.ema_price0_nad(),
            GetterType::EmaPrice1Nad => pair.ema_price1_nad(),
            GetterType::SpotPrice0Nad => pair.spot_price0_nad(),
            GetterType::SpotPrice1Nad => pair.spot_price1_nad(),
        };

        msg!("{}: {}", getter, value);

        Ok(())
    }
}