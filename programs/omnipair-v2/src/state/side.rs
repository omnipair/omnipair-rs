use anchor_lang::prelude::*;

use super::{DailyLimits, Fees, ReserveShares, Reserves};
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketAsset {
    Base,
    Quote,
}

impl MarketAsset {
    pub fn code(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Quote => 1,
        }
    }

    pub fn try_from_code(code: u8) -> Result<Self> {
        match code {
            0 => Ok(Self::Base),
            1 => Ok(Self::Quote),
            _ => err!(ErrorCode::InvalidArgument),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Base => Self::Quote,
            Self::Quote => Self::Base,
        }
    }

    pub fn is_base(self) -> bool {
        matches!(self, Self::Base)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSide {
    pub asset_mint: Pubkey,
    pub asset_decimals: u8,
    pub ylp_mint: Pubkey,
    pub hlp_mint: Pubkey,
    pub reserve_vault: Pubkey,
    pub collateral_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub interest_vault: Pubkey,
    pub reserves: Reserves,
    pub shares: ReserveShares,
    pub fees: Fees,
    pub daily_limits: DailyLimits,
}

impl MarketSide {
    pub fn assert_share_backing(&self) -> Result<()> {
        if self.shares.ylp_supply == 0 {
            require_eq!(self.reserves.live_reserve, 0, ErrorCode::BrokenInvariant);
        }
        require_gte!(
            self.reserves.live_reserve,
            self.reserves.reserved_liability,
            ErrorCode::InsufficientLiquidity
        );
        Ok(())
    }

    pub fn ylp_exchange_rate_nad(&self) -> Result<u128> {
        if self.shares.ylp_supply == 0 {
            return Ok(0);
        }
        (self.reserves.live_reserve as u128)
            .checked_mul(crate::constants::NAD as u128)
            .and_then(|value| value.checked_div(self.shares.ylp_supply as u128))
            .ok_or(ErrorCode::MarketMathOverflow.into())
    }
}
