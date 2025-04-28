use anchor_lang::prelude::*;
use crate::constants::*;

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

    pub fn key(&self) -> Pubkey {
        Pubkey::find_program_address(&[
            POSITION_SEED_PREFIX,
            self.pair.as_ref(),
            self.owner.as_ref(),
        ], &crate::ID).0
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