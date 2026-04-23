use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserLeveragePosition {
    pub owner: Pubkey,
    pub pair: Pubkey,
    pub is_lev_collateral0: bool,
    pub lev_collateral_amount: u64,
    pub multiplier_bps: u64,
    pub position_size: u64,
    pub borrow_amount: u64,
    pub debt_shares: u128,
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
        position_size: u64,
        borrow_amount: u64,
        debt_shares: u128,
        opened_at: i64,
        bump: u8,
    ) {
        self.owner = owner;
        self.pair = pair;
        self.is_lev_collateral0 = is_lev_collateral0;
        self.lev_collateral_amount = lev_collateral_amount;
        self.multiplier_bps = multiplier_bps;
        self.position_size = position_size;
        self.borrow_amount = borrow_amount;
        self.debt_shares = debt_shares;
        self.opened_at = opened_at;
        self.bump = bump;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_sets_fields() {
        let owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let mut p = UserLeveragePosition {
            owner: Pubkey::default(),
            pair: Pubkey::default(),
            is_lev_collateral0: false,
            lev_collateral_amount: 0,
            multiplier_bps: 0,
            position_size: 0,
            borrow_amount: 0,
            debt_shares: 0,
            opened_at: 0,
            bump: 0,
        };

        p.initialize(
            owner,
            pair,
            true,
            100,
            20_000,
            150,
            50,
            50_000_000,
            1_700_000_000,
            255,
        );

        assert_eq!(p.owner, owner);
        assert_eq!(p.pair, pair);
        assert!(p.is_lev_collateral0);
        assert_eq!(p.lev_collateral_amount, 100);
        assert_eq!(p.multiplier_bps, 20_000);
        assert_eq!(p.position_size, 150);
        assert_eq!(p.borrow_amount, 50);
        assert_eq!(p.debt_shares, 50_000_000);
        assert_eq!(p.opened_at, 1_700_000_000);
        assert_eq!(p.bump, 255);
    }
}
