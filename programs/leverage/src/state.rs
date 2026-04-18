use anchor_lang::prelude::*;

pub const LEVERAGE_POSITION_SEED_PREFIX: &[u8] = b"leverage_position";

/// Tracks metadata for a user's leveraged position opened through this program.
///
/// One account per (pair, user). The actual collateral and debt amounts are
/// owned by omnipair's `UserPosition` — this account records the leverage-specific
/// context (direction, entry multiplier, amounts at open time) so clients can
/// compute PnL, health, and display the position without re-deriving history.
///
/// `position_size` is written by the wrapper after Omnipair's native leverage
/// instruction completes, because the exact amount is only known post-execution.
#[account]
#[derive(InitSpace)]
pub struct UserLeveragePosition {
    /// User wallet that opened this position
    pub owner: Pubkey,
    /// The omnipair pair this leverage position is on
    pub pair: Pubkey,
    /// Direction: true = user is long token0 (borrowed token0 to buy token1 collateral).
    /// false = long token1.
    pub is_lev_collateral0: bool,
    /// User's own capital deposited at open (the non-borrowed portion), in lev_collateral token
    pub lev_collateral_amount: u64,
    /// Leverage multiplier at open (BPS, e.g. 20_000 = 2×)
    pub multiplier_bps: u64,
    /// Amount of position token deposited as collateral (swap output, set after native execution)
    pub position_size: u64,
    /// Principal borrowed from omnipair at open (excludes flashloan fee)
    pub borrow_amount: u64,
    /// Unix timestamp when this position was last opened / topped-up
    pub opened_at: i64,
    pub bump: u8,
}

impl UserLeveragePosition {
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        pair: Pubkey,
        is_lev_collateral0: bool,
        lev_collateral_amount: u64,
        multiplier_bps: u64,
        borrow_amount: u64,
        opened_at: i64,
        bump: u8,
    ) {
        self.owner = owner;
        self.pair = pair;
        self.is_lev_collateral0 = is_lev_collateral0;
        self.lev_collateral_amount = lev_collateral_amount;
        self.multiplier_bps = multiplier_bps;
        self.position_size = 0; // set after native Omnipair execution
        self.borrow_amount = borrow_amount;
        self.opened_at = opened_at;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_sets_fields_and_zero_position_size() {
        let owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let mut p = UserLeveragePosition {
            owner: Pubkey::default(),
            pair: Pubkey::default(),
            is_lev_collateral0: false,
            lev_collateral_amount: 0,
            multiplier_bps: 0,
            position_size: 999,
            borrow_amount: 0,
            opened_at: 0,
            bump: 0,
        };
        assert!(!p.is_initialized());
        p.initialize(owner, pair, true, 100, 20_000, 50, 1_700_000_000, 255);
        assert_eq!(p.owner, owner);
        assert_eq!(p.pair, pair);
        assert!(p.is_lev_collateral0);
        assert_eq!(p.lev_collateral_amount, 100);
        assert_eq!(p.multiplier_bps, 20_000);
        assert_eq!(p.position_size, 0);
        assert_eq!(p.borrow_amount, 50);
        assert_eq!(p.opened_at, 1_700_000_000);
        assert_eq!(p.bump, 255);
        assert!(p.is_initialized());
    }
}
