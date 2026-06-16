use anchor_lang::prelude::*;

use crate::{
    errors::ErrorCode,
    state::{MarketSide, StakePosition},
    transitions::fee::CarryForwardStakerFees,
};

pub struct Stake {
    pub claim_amount: u64,
    pub buffer_share_amount: u64,
}

pub struct Unstake {
    pub claim_amount: u64,
    pub buffer_share_amount: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StakeReceipt {
    pub active_stake_units: u64,
    pub accrued_fee_amount: u64,
    pub staked_claim_token_amount: u64,
    pub staked_buffer_share_amount: u64,
}

impl Stake {
    pub fn new(claim_amount: u64, buffer_share_amount: u64) -> Self {
        Self {
            claim_amount,
            buffer_share_amount,
        }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        stake_position: &mut StakePosition,
    ) -> Result<StakeReceipt> {
        CarryForwardStakerFees.apply(market_side)?;
        stake_position.accrue_fees(
            market_side.fee_ledger.fee_growth_index_nad,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        stake_position.stake(self.claim_amount, self.buffer_share_amount)?;
        market_side.claim_token_ledger.staked_claim_token_supply = market_side
            .claim_token_ledger
            .staked_claim_token_supply
            .checked_add(self.claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.buffer_ledger.staked_buffer_share_amount = market_side
            .buffer_ledger
            .staked_buffer_share_amount
            .checked_add(self.buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        CarryForwardStakerFees.apply(market_side)?;
        StakeReceipt::from_position(market_side, stake_position)
    }
}

impl Unstake {
    pub fn new(claim_amount: u64, buffer_share_amount: u64) -> Self {
        Self {
            claim_amount,
            buffer_share_amount,
        }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        stake_position: &mut StakePosition,
    ) -> Result<StakeReceipt> {
        CarryForwardStakerFees.apply(market_side)?;
        stake_position.accrue_fees(
            market_side.fee_ledger.fee_growth_index_nad,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        stake_position.unstake(self.claim_amount, self.buffer_share_amount)?;
        market_side.claim_token_ledger.staked_claim_token_supply = market_side
            .claim_token_ledger
            .staked_claim_token_supply
            .checked_sub(self.claim_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.buffer_ledger.staked_buffer_share_amount = market_side
            .buffer_ledger
            .staked_buffer_share_amount
            .checked_sub(self.buffer_share_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        StakeReceipt::from_position(market_side, stake_position)
    }
}

impl StakeReceipt {
    fn from_position(
        market_side: &MarketSide,
        stake_position: &StakePosition,
    ) -> Result<StakeReceipt> {
        Ok(StakeReceipt {
            active_stake_units: stake_position
                .active_stake_units(market_side.buffer_ledger.buffer_ratio_bps)?,
            accrued_fee_amount: stake_position.accrued_fee_amount,
            staked_claim_token_amount: stake_position.staked_claim_token_amount,
            staked_buffer_share_amount: stake_position.staked_buffer_share_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BufferLedger, ClaimTokenLedger};

    fn market_side() -> MarketSide {
        MarketSide {
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn stake_position() -> StakePosition {
        StakePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            available_buffer_share_amount: 0,
            staked_claim_token_amount: 0,
            staked_buffer_share_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    #[test]
    fn stake_updates_position_and_side_supplies() {
        let mut market_side = market_side();
        let mut stake_position = stake_position();
        stake_position.available_buffer_share_amount = 200;

        let receipt = Stake::new(800, 200)
            .apply(&mut market_side, &mut stake_position)
            .unwrap();

        assert_eq!(receipt.active_stake_units, 1_000);
        assert_eq!(receipt.staked_claim_token_amount, 800);
        assert_eq!(receipt.staked_buffer_share_amount, 200);
        assert_eq!(
            market_side.claim_token_ledger.staked_claim_token_supply,
            800
        );
        assert_eq!(market_side.buffer_ledger.staked_buffer_share_amount, 200);
    }

    #[test]
    fn stake_carries_forward_waiting_fees() {
        let mut market_side = market_side();
        market_side.claim_token_ledger = ClaimTokenLedger {
            protected_claim_token_supply: 800,
            ..ClaimTokenLedger::default()
        };
        market_side.fee_ledger.fee_vault_balance = 100;
        market_side.fee_ledger.unallocated_fee_liability = 100;
        let mut stake_position = stake_position();
        stake_position.available_buffer_share_amount = 200;

        Stake::new(800, 200)
            .apply(&mut market_side, &mut stake_position)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_liability, 100);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
    }

    #[test]
    fn unstake_updates_position_and_side_supplies() {
        let mut market_side = market_side();
        market_side.claim_token_ledger.staked_claim_token_supply = 800;
        market_side.buffer_ledger.staked_buffer_share_amount = 200;
        let mut stake_position = stake_position();
        stake_position.staked_claim_token_amount = 800;
        stake_position.staked_buffer_share_amount = 200;

        let receipt = Unstake::new(300, 75)
            .apply(&mut market_side, &mut stake_position)
            .unwrap();

        assert_eq!(receipt.staked_claim_token_amount, 500);
        assert_eq!(receipt.staked_buffer_share_amount, 125);
        assert_eq!(stake_position.available_buffer_share_amount, 75);
        assert_eq!(
            market_side.claim_token_ledger.staked_claim_token_supply,
            500
        );
        assert_eq!(market_side.buffer_ledger.staked_buffer_share_amount, 125);
    }
}
