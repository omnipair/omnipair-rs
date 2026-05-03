use anchor_lang::prelude::*;

use crate::{constants::DEBT_SHARE_SCALE, errors::ErrorCode, utils::math::ceil_div};

use super::Pair;

#[account]
#[derive(InitSpace)]
pub struct UserLeveragePosition {
    pub owner: Pubkey,
    pub pair: Pubkey,
    pub is_debt_token0: bool,
    pub collateral_amount: u64,
    pub margin_amount: u64,
    pub open_notional: u64,
    pub debt_amount: u64,
    pub debt_shares: u128,
    pub multiplier_bps: u64,
    pub opened_at: i64,
    pub opened_slot: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct UserLeverageDelegation {
    pub owner: Pubkey,
    pub pair: Pubkey,
    pub position: Pubkey,
    pub is_debt_token0: bool,
    pub delegated_program: Pubkey,
    pub approved_actions: u32,
    pub bump: u8,
}

impl UserLeverageDelegation {
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        pair: Pubkey,
        position: Pubkey,
        is_debt_token0: bool,
        delegated_program: Pubkey,
        approved_actions: u32,
        bump: u8,
    ) {
        self.owner = owner;
        self.pair = pair;
        self.position = position;
        self.is_debt_token0 = is_debt_token0;
        self.delegated_program = delegated_program;
        self.approved_actions = approved_actions;
        self.bump = bump;
    }

    pub fn update(&mut self, delegated_program: Pubkey, approved_actions: u32) {
        self.delegated_program = delegated_program;
        self.approved_actions = approved_actions;
    }
}

impl UserLeveragePosition {
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        pair: Pubkey,
        is_debt_token0: bool,
        collateral_amount: u64,
        margin_amount: u64,
        open_notional: u64,
        debt_amount: u64,
        debt_shares: u128,
        multiplier_bps: u64,
        opened_at: i64,
        opened_slot: u64,
        bump: u8,
    ) {
        self.owner = owner;
        self.pair = pair;
        self.is_debt_token0 = is_debt_token0;
        self.collateral_amount = collateral_amount;
        self.margin_amount = margin_amount;
        self.open_notional = open_notional;
        self.debt_amount = debt_amount;
        self.debt_shares = debt_shares;
        self.multiplier_bps = multiplier_bps;
        self.opened_at = opened_at;
        self.opened_slot = opened_slot;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.pair != Pubkey::default()
    }

    pub fn calculate_debt(&self, pair: &Pair) -> Result<u64> {
        let (total_debt, total_debt_shares) = match self.is_debt_token0 {
            true => (pair.total_debt0, pair.total_debt0_shares),
            false => (pair.total_debt1, pair.total_debt1_shares),
        };

        match total_debt_shares {
            0 => Ok(0),
            _ => Ok(ceil_div(
                self.debt_shares
                    .checked_mul(total_debt as u128)
                    .ok_or(ErrorCode::DebtMathOverflow)?,
                total_debt_shares,
            )
            .ok_or(ErrorCode::DebtShareDivisionOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::DebtShareDivisionOverflow)?),
        }
    }

    pub fn increase_debt(&mut self, pair: &mut Pair, amount: u64) -> Result<()> {
        let shares = match self.is_debt_token0 {
            true => {
                let shares = match pair.total_debt0_shares {
                    0 => (amount as u128)
                        .checked_mul(DEBT_SHARE_SCALE as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?,
                    _ => ceil_div(
                        (amount as u128)
                            .checked_mul(pair.total_debt0_shares)
                            .ok_or(ErrorCode::DebtShareMathOverflow)?,
                        pair.total_debt0 as u128,
                    )
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?,
                };
                pair.total_debt0_shares = pair.total_debt0_shares.saturating_add(shares);
                pair.total_debt0 = pair.total_debt0.saturating_add(amount);
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_sub(amount)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
                shares
            }
            false => {
                let shares = match pair.total_debt1_shares {
                    0 => (amount as u128)
                        .checked_mul(DEBT_SHARE_SCALE as u128)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?,
                    _ => ceil_div(
                        (amount as u128)
                            .checked_mul(pair.total_debt1_shares)
                            .ok_or(ErrorCode::DebtShareMathOverflow)?,
                        pair.total_debt1 as u128,
                    )
                    .ok_or(ErrorCode::DebtShareDivisionOverflow)?,
                };
                pair.total_debt1_shares = pair.total_debt1_shares.saturating_add(shares);
                pair.total_debt1 = pair.total_debt1.saturating_add(amount);
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_sub(amount)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
                shares
            }
        };

        self.debt_shares = self
            .debt_shares
            .checked_add(shares)
            .ok_or(ErrorCode::DebtShareMathOverflow)?;
        Ok(())
    }

    pub fn decrease_debt(&mut self, pair: &mut Pair, amount: u64) -> Result<u128> {
        self.reduce_debt(pair, amount, true)
    }

    pub fn reduce_debt_from_closeout(&mut self, pair: &mut Pair, amount: u64) -> Result<u128> {
        self.reduce_debt(pair, amount, false)
    }

    fn reduce_debt(&mut self, pair: &mut Pair, amount: u64, add_cash: bool) -> Result<u128> {
        require!(amount > 0, ErrorCode::AmountZero);

        let current_debt = self.calculate_debt(pair)?;
        require_gte!(current_debt, amount, ErrorCode::InsufficientDebt);

        let shares = match self.is_debt_token0 {
            true => {
                require_gte!(pair.total_debt0, amount, ErrorCode::DebtMathOverflow);
                if amount >= pair.total_debt0 && self.debt_shares < pair.total_debt0_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }

                let shares = if amount == current_debt {
                    self.debt_shares
                } else {
                    let shares = (amount as u128)
                        .checked_mul(pair.total_debt0_shares)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt0 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?;
                    require!(shares > 0, ErrorCode::DebtShareDivisionOverflow);
                    shares.min(self.debt_shares)
                };

                pair.total_debt0_shares = pair.total_debt0_shares.saturating_sub(shares);
                pair.total_debt0 = pair.total_debt0.saturating_sub(amount);
                if add_cash {
                    pair.cash_reserve0 = pair
                        .cash_reserve0
                        .checked_add(amount)
                        .ok_or(ErrorCode::Overflow)?;
                }
                if pair.total_debt0_shares == 0 {
                    pair.total_debt0 = 0;
                }
                if pair.total_debt0 == 0 {
                    pair.total_debt0_shares = 0;
                }
                shares
            }
            false => {
                require_gte!(pair.total_debt1, amount, ErrorCode::DebtMathOverflow);
                if amount >= pair.total_debt1 && self.debt_shares < pair.total_debt1_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }

                let shares = if amount == current_debt {
                    self.debt_shares
                } else {
                    let shares = (amount as u128)
                        .checked_mul(pair.total_debt1_shares)
                        .ok_or(ErrorCode::DebtShareMathOverflow)?
                        .checked_div(pair.total_debt1 as u128)
                        .ok_or(ErrorCode::DebtShareDivisionOverflow)?;
                    require!(shares > 0, ErrorCode::DebtShareDivisionOverflow);
                    shares.min(self.debt_shares)
                };

                pair.total_debt1_shares = pair.total_debt1_shares.saturating_sub(shares);
                pair.total_debt1 = pair.total_debt1.saturating_sub(amount);
                if add_cash {
                    pair.cash_reserve1 = pair
                        .cash_reserve1
                        .checked_add(amount)
                        .ok_or(ErrorCode::Overflow)?;
                }
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1 = 0;
                }
                if pair.total_debt1 == 0 {
                    pair.total_debt1_shares = 0;
                }
                shares
            }
        };

        self.debt_shares = self.debt_shares.saturating_sub(shares);
        self.debt_amount = self.debt_amount.saturating_sub(amount);
        Ok(shares)
    }

    pub fn clear_debt(&mut self, pair: &mut Pair, debt_amount: u64) -> Result<()> {
        match self.is_debt_token0 {
            true => {
                require_gte!(
                    pair.total_debt0_shares,
                    self.debt_shares,
                    ErrorCode::DebtShareMathOverflow
                );
                require_gte!(
                    pair.total_debt0,
                    debt_amount,
                    ErrorCode::DebtMathOverflow
                );
                if debt_amount >= pair.total_debt0 && self.debt_shares < pair.total_debt0_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }
                pair.total_debt0_shares = pair.total_debt0_shares.saturating_sub(self.debt_shares);
                pair.total_debt0 = pair.total_debt0.saturating_sub(debt_amount);
                if pair.total_debt0_shares == 0 {
                    pair.total_debt0 = 0;
                }
                if pair.total_debt0 == 0 {
                    pair.total_debt0_shares = 0;
                }
            }
            false => {
                require_gte!(
                    pair.total_debt1_shares,
                    self.debt_shares,
                    ErrorCode::DebtShareMathOverflow
                );
                require_gte!(
                    pair.total_debt1,
                    debt_amount,
                    ErrorCode::DebtMathOverflow
                );
                if debt_amount >= pair.total_debt1 && self.debt_shares < pair.total_debt1_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }
                pair.total_debt1_shares = pair.total_debt1_shares.saturating_sub(self.debt_shares);
                pair.total_debt1 = pair.total_debt1.saturating_sub(debt_amount);
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1 = 0;
                }
                if pair.total_debt1 == 0 {
                    pair.total_debt1_shares = 0;
                }
            }
        }
        self.debt_shares = 0;
        self.debt_amount = 0;
        Ok(())
    }

    pub fn writeoff_debt_shares(
        &mut self,
        pair: &mut Pair,
        shares: u128,
        amount: u64,
    ) -> Result<()> {
        require!(shares > 0, ErrorCode::DebtShareDivisionOverflow);
        require!(amount > 0, ErrorCode::DebtMathOverflow);
        require_gte!(self.debt_shares, shares, ErrorCode::DebtShareMathOverflow);

        match self.is_debt_token0 {
            true => {
                require_gte!(pair.total_debt0_shares, shares, ErrorCode::DebtShareMathOverflow);
                require_gte!(pair.total_debt0, amount, ErrorCode::DebtMathOverflow);
                if amount >= pair.total_debt0 && shares < pair.total_debt0_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }
                pair.total_debt0_shares = pair.total_debt0_shares.saturating_sub(shares);
                pair.total_debt0 = pair.total_debt0.saturating_sub(amount);
                pair.reserve0 = pair
                    .reserve0
                    .checked_sub(amount)
                    .ok_or(ErrorCode::ReserveUnderflow)?;
                if pair.total_debt0_shares == 0 {
                    pair.total_debt0 = 0;
                }
                if pair.total_debt0 == 0 {
                    pair.total_debt0_shares = 0;
                }
            }
            false => {
                require_gte!(pair.total_debt1_shares, shares, ErrorCode::DebtShareMathOverflow);
                require_gte!(pair.total_debt1, amount, ErrorCode::DebtMathOverflow);
                if amount >= pair.total_debt1 && shares < pair.total_debt1_shares {
                    return err!(ErrorCode::DebtShareDivisionOverflow);
                }
                pair.total_debt1_shares = pair.total_debt1_shares.saturating_sub(shares);
                pair.total_debt1 = pair.total_debt1.saturating_sub(amount);
                pair.reserve1 = pair
                    .reserve1
                    .checked_sub(amount)
                    .ok_or(ErrorCode::ReserveUnderflow)?;
                if pair.total_debt1_shares == 0 {
                    pair.total_debt1 = 0;
                }
                if pair.total_debt1 == 0 {
                    pair.total_debt1_shares = 0;
                }
            }
        }

        self.debt_shares = self.debt_shares.saturating_sub(shares);
        self.debt_amount = self.debt_amount.saturating_sub(amount);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_with_debt(total_debt0: u64, total_debt0_shares: u128) -> Pair {
        Pair {
            token0: Pubkey::new_unique(),
            token1: Pubkey::new_unique(),
            lp_mint: Pubkey::new_unique(),
            rate_model: Pubkey::new_unique(),
            swap_fee_bps: 0,
            half_life: 0,
            fixed_cf_bps: None,
            reserve0: 1_000_000,
            reserve1: 1_000_000,
            cash_reserve0: 1_000_000u64.saturating_sub(total_debt0),
            cash_reserve1: 1_000_000,
            last_price0_ema: Default::default(),
            last_price1_ema: Default::default(),
            last_update: 0,
            last_rate0: 0,
            last_rate1: 0,
            total_debt0,
            total_debt1: 0,
            total_debt0_shares,
            total_debt1_shares: 0,
            total_supply: 1_000,
            total_collateral0: 0,
            total_collateral1: 0,
            token0_decimals: 6,
            token1_decimals: 6,
            params_hash: [0; 32],
            version: 1,
            bump: 255,
            vault_bumps: Default::default(),
            reduce_only: false,
        }
    }

    fn empty_position(is_debt_token0: bool) -> UserLeveragePosition {
        UserLeveragePosition {
            owner: Pubkey::new_unique(),
            pair: Pubkey::new_unique(),
            is_debt_token0,
            collateral_amount: 0,
            margin_amount: 0,
            open_notional: 0,
            debt_amount: 0,
            debt_shares: 0,
            multiplier_bps: 0,
            opened_at: 0,
            opened_slot: 0,
            bump: 0,
        }
    }

    #[test]
    fn delegation_initialize_and_update_store_authority_fields() {
        let owner = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let position = Pubkey::new_unique();
        let first_program = Pubkey::new_unique();
        let next_program = Pubkey::new_unique();
        let mut delegation = UserLeverageDelegation {
            owner: Pubkey::default(),
            pair: Pubkey::default(),
            position: Pubkey::default(),
            is_debt_token0: false,
            delegated_program: Pubkey::default(),
            approved_actions: 0,
            bump: 0,
        };

        delegation.initialize(owner, pair, position, true, first_program, 3, 254);
        assert_eq!(delegation.owner, owner);
        assert_eq!(delegation.pair, pair);
        assert_eq!(delegation.position, position);
        assert!(delegation.is_debt_token0);
        assert_eq!(delegation.delegated_program, first_program);
        assert_eq!(delegation.approved_actions, 3);
        assert_eq!(delegation.bump, 254);

        delegation.update(next_program, 16);
        assert_eq!(delegation.delegated_program, next_program);
        assert_eq!(delegation.approved_actions, 16);
        assert_eq!(delegation.owner, owner);
        assert_eq!(delegation.position, position);
    }

    #[test]
    fn calculate_debt_ceil_rounds_dust_up() {
        let pair = pair_with_debt(1, 6);
        let mut position = empty_position(true);
        position.debt_shares = 1;

        assert_eq!(position.calculate_debt(&pair).unwrap(), 1);
    }

    #[test]
    fn calculate_debt_returns_zero_when_pool_has_no_shares() {
        let pair = pair_with_debt(100, 0);
        let mut position = empty_position(true);
        position.debt_shares = 1;

        assert_eq!(position.calculate_debt(&pair).unwrap(), 0);
    }

    #[test]
    fn first_debt_mint_uses_scaled_shares() {
        let mut pair = pair_with_debt(0, 0);
        let mut position = empty_position(true);

        position.increase_debt(&mut pair, 100).unwrap();

        assert_eq!(position.debt_shares, 100_000_000);
        assert_eq!(pair.total_debt0, 100);
        assert_eq!(pair.total_debt0_shares, 100_000_000);
        assert_eq!(pair.cash_reserve0, 999_900);
    }

    #[test]
    fn subsequent_debt_mint_tracks_existing_share_price() {
        let mut pair = pair_with_debt(100, 100_000_000);
        let mut position = empty_position(true);

        position.increase_debt(&mut pair, 50).unwrap();

        assert_eq!(position.debt_shares, 50_000_000);
        assert_eq!(pair.total_debt0, 150);
        assert_eq!(pair.total_debt0_shares, 150_000_000);
    }

    #[test]
    fn increase_debt_rejects_cash_underflow() {
        let mut pair = pair_with_debt(0, 0);
        pair.cash_reserve0 = 50;
        let mut position = empty_position(true);

        assert!(position.increase_debt(&mut pair, 100).is_err());
    }

    #[test]
    fn token1_debt_path_updates_token1_totals_and_cash() {
        let mut pair = pair_with_debt(0, 0);
        let mut position = empty_position(false);

        position.increase_debt(&mut pair, 100).unwrap();
        assert_eq!(position.debt_shares, 100_000_000);
        assert_eq!(pair.total_debt1, 100);
        assert_eq!(pair.total_debt1_shares, 100_000_000);
        assert_eq!(pair.cash_reserve1, 999_900);
        assert_eq!(position.calculate_debt(&pair).unwrap(), 100);

        position.debt_amount = 100;
        position.decrease_debt(&mut pair, 25).unwrap();
        assert_eq!(position.debt_amount, 75);
        assert_eq!(position.debt_shares, 75_000_000);
        assert_eq!(pair.total_debt1, 75);
        assert_eq!(pair.total_debt1_shares, 75_000_000);
        assert_eq!(pair.cash_reserve1, 999_925);
    }

    #[test]
    fn full_clear_removes_position_shares_without_desync() {
        let mut pair = pair_with_debt(100, 100_000_000);
        let mut position = empty_position(true);
        position.debt_amount = 100;
        position.debt_shares = 100_000_000;

        position.clear_debt(&mut pair, 100).unwrap();

        assert_eq!(position.debt_shares, 0);
        assert_eq!(pair.total_debt0, 0);
        assert_eq!(pair.total_debt0_shares, 0);
    }

    #[test]
    fn clear_rejects_debt_zero_with_remaining_shares() {
        let mut pair = pair_with_debt(1, 6);
        let mut position = empty_position(true);
        position.debt_amount = 1;
        position.debt_shares = 1;

        assert!(position.clear_debt(&mut pair, 1).is_err());
        assert_eq!(pair.total_debt0, 1);
        assert_eq!(pair.total_debt0_shares, 6);
    }

    #[test]
    fn external_repay_decreases_debt_and_adds_cash() {
        let mut pair = pair_with_debt(100, 100_000_000);
        let mut position = empty_position(true);
        position.debt_amount = 100;
        position.debt_shares = 100_000_000;

        position.decrease_debt(&mut pair, 25).unwrap();

        assert_eq!(position.debt_amount, 75);
        assert_eq!(position.debt_shares, 75_000_000);
        assert_eq!(pair.total_debt0, 75);
        assert_eq!(pair.total_debt0_shares, 75_000_000);
        assert_eq!(pair.cash_reserve0, 999_925);
    }

    #[test]
    fn closeout_repay_decreases_debt_without_adding_cash() {
        let mut pair = pair_with_debt(100, 100_000_000);
        let mut position = empty_position(true);
        position.debt_amount = 100;
        position.debt_shares = 100_000_000;

        position.reduce_debt_from_closeout(&mut pair, 25).unwrap();

        assert_eq!(position.debt_amount, 75);
        assert_eq!(position.debt_shares, 75_000_000);
        assert_eq!(pair.total_debt0, 75);
        assert_eq!(pair.total_debt0_shares, 75_000_000);
        assert_eq!(pair.cash_reserve0, 999_900);
    }

    #[test]
    fn liquidation_writeoff_reduces_exact_shares_without_adding_cash() {
        let mut pair = pair_with_debt(100, 100_000_000);
        let mut position = empty_position(true);
        position.debt_amount = 100;
        position.debt_shares = 100_000_000;

        position
            .writeoff_debt_shares(&mut pair, 50_000_000, 50)
            .unwrap();

        assert_eq!(position.debt_amount, 50);
        assert_eq!(position.debt_shares, 50_000_000);
        assert_eq!(pair.total_debt0, 50);
        assert_eq!(pair.total_debt0_shares, 50_000_000);
        assert_eq!(pair.reserve0, 999_950);
        assert_eq!(pair.cash_reserve0, 999_900);
    }

    #[test]
    fn liquidation_writeoff_rejects_zero_debt_for_nonzero_shares() {
        let mut pair = pair_with_debt(1, 6);
        let mut position = empty_position(true);
        position.debt_amount = 1;
        position.debt_shares = 1;

        assert!(position.writeoff_debt_shares(&mut pair, 1, 0).is_err());
        assert_eq!(position.debt_shares, 1);
        assert_eq!(pair.total_debt0, 1);
        assert_eq!(pair.total_debt0_shares, 6);
    }

    #[test]
    fn partial_decrease_rejects_zero_share_repayment() {
        let mut pair = pair_with_debt(10_000_000, 10);
        let mut position = empty_position(true);
        position.debt_amount = 1_000_000;
        position.debt_shares = 1;

        assert!(position.decrease_debt(&mut pair, 1).is_err());
        assert_eq!(position.debt_shares, 1);
        assert_eq!(pair.total_debt0, 10_000_000);
        assert_eq!(pair.total_debt0_shares, 10);
    }
}
