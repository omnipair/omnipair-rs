use anchor_lang::prelude::*;
use crate::constants::*;
use crate::utils::gamm_math::max_borrowable_with_safety;
use super::Pair;

#[account]
pub struct UserPosition {
    // User and pair info
    pub owner: Pubkey,         // who owns this position
    pub pair: Pubkey,          // the pair this position belongs to
    
    // Collateral tracking
    pub collateral0: u64,      // token0 collateral amount
    pub collateral1: u64,      // token1 collateral amount
    
    // Debt tracking
    pub debt0_shares: u64,     // debt shares for token0
    pub debt1_shares: u64,     // debt shares for token1

    // PDA bump
    pub bump: u8,
}

impl UserPosition {
    pub fn initialize(
        owner: Pubkey,
        pair: Pubkey,
        bump: u8,
    ) -> Self {
        Self {
            owner,
            pair,
            bump,
            collateral0: 0,
            collateral1: 0,
            debt0_shares: 0,
            debt1_shares: 0,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.pair != Pubkey::default()
    }

    pub fn seeds<'a>(&'a self) -> [&'a [u8]; 3] {
        [
            POSITION_SEED_PREFIX,
            self.pair.as_ref(),
            self.owner.as_ref(),
        ]
    }

    pub fn calculate_debt0(&self, total_debt0: u64, total_debt0_shares: u64) -> u64 {
        match total_debt0_shares {
            0 => 0,
            _ => self.debt0_shares
                .checked_mul(total_debt0).unwrap()
                .checked_div(total_debt0_shares).unwrap()
        }
    }

    pub fn calculate_debt1(&self, total_debt1: u64, total_debt1_shares: u64) -> u64 {
        match total_debt1_shares {
            0 => 0,
            _ => self.debt1_shares
                .checked_mul(total_debt1).unwrap()
                .checked_div(total_debt1_shares).unwrap()
        }
    }

    pub fn get_borrowing_power_and_effective_cf_bps(&self, pair: &Pair, token: &Pubkey) -> (u64, u16) {
        let user_position = &self;

        let (
            user_collateral, 
            collateral_spot_price,
            collateral_ema_price,
            // in token X (debt token)
            pair_debt_reserve,
            pair_total_debt,
        ) = match *token == pair.token0 {
            true => (
                user_position.collateral1,
                pair.spot_price1_nad(),
                pair.ema_price1_nad(),
                pair.reserve0,
                pair.total_debt0,
            ),
            false => (
                user_position.collateral0,
                pair.spot_price0_nad(),
                pair.ema_price0_nad(),
                pair.reserve1,
                pair.total_debt1,
            )
        };       

        // TODO: normalize collateral token decimals to NAD if needed
        // let NormalizedTwoValues { 
        //     scaled_a: user_collateral_scaled, 
        //     scaled_b: collateral_spot_price_scaled 
        // } = normalize_two_values_to_nad(
        //     user_collateral,
        //     collateral_decimals,
        //     collateral_spot_price,
        // );

        max_borrowable_with_safety(
            user_collateral,
            collateral_ema_price,
            collateral_spot_price,
            pair_total_debt,
            pair_debt_reserve,
        )
    }

    pub fn get_borrowing_power(&self, pair: &Pair, token: &Pubkey) -> u64 {
        self.get_borrowing_power_and_effective_cf_bps(pair, token).0
    }

    pub fn get_effective_collateral_factor_bps(&self, pair: &Pair, token: &Pubkey) -> u64 {
        self.get_borrowing_power_and_effective_cf_bps(pair, token).1 as u64
    }
    
}

#[macro_export]
macro_rules! generate_user_position_seeds {
    ($position:expr) => {{
        &[
            USER_POSITION_SEED_PREFIX,
            $position.pair.as_ref(),
            $position.owner.as_ref(),
            &[$position.bump],
        ]
    }};
} 