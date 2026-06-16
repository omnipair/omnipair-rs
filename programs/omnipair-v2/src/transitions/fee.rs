use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, NAD, TAYLOR_TERMS},
    errors::ErrorCode,
    shared::math::taylor_exp,
    state::{HedgePosition, MarketFeeClaimKind, MarketSide, StakePosition},
    utils::market_math::active_stake_units,
};

pub struct RecordFeeCredit {
    pub fee_credit: u64,
    pub operator_fee_bps: u16,
    pub protocol_fee_bps: u16,
    pub fee_routing_k_nad: u64,
}

pub struct CarryForwardStakerFees;

pub struct CarryForwardHedgedFees;

pub struct PrepareStakerFeeClaim {
    pub fee_vault_balance: u64,
}

pub struct SettleStakerFeeClaim {
    pub fee_amount: u64,
    pub fee_vault_balance: u64,
}

pub struct PrepareHedgedFeeClaim {
    pub fee_vault_balance: u64,
}

pub struct SettleHedgedFeeClaim {
    pub fee_amount: u64,
    pub fee_vault_balance: u64,
}

pub struct PrepareMarketFeeClaim {
    pub claim_kind: MarketFeeClaimKind,
    pub fee_vault_balance: u64,
}

pub struct SettleMarketFeeClaim {
    pub claim_kind: MarketFeeClaimKind,
    pub fee_amount: u64,
    pub fee_vault_balance: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeeLedgerReceipt {
    pub fee_growth_index_nad: u128,
    pub hedged_fee_growth_index_nad: u128,
    pub fee_liability: u64,
    pub hedged_fee_liability: u64,
    pub unallocated_fee_liability: u64,
    pub unallocated_hedged_fee_liability: u64,
    pub operator_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub fee_vault_balance: u64,
}

impl FeeLedgerReceipt {
    pub fn from_side(market_side: &MarketSide) -> Self {
        let ledger = &market_side.fee_ledger;
        Self {
            fee_growth_index_nad: ledger.fee_growth_index_nad,
            hedged_fee_growth_index_nad: ledger.hedged_fee_growth_index_nad,
            fee_liability: ledger.fee_liability,
            hedged_fee_liability: ledger.hedged_fee_liability,
            unallocated_fee_liability: ledger.unallocated_fee_liability,
            unallocated_hedged_fee_liability: ledger.unallocated_hedged_fee_liability,
            operator_fee_liability: ledger.operator_fee_liability,
            protocol_fee_liability: ledger.protocol_fee_liability,
            fee_vault_balance: ledger.fee_vault_balance,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeeClaimReceipt {
    pub fee_amount: u64,
    pub remaining_fee_liability: u64,
    pub fee_vault_balance: u64,
}

impl RecordFeeCredit {
    pub fn new(
        fee_credit: u64,
        operator_fee_bps: u16,
        protocol_fee_bps: u16,
        fee_routing_k_nad: u64,
    ) -> Self {
        Self {
            fee_credit,
            operator_fee_bps,
            protocol_fee_bps,
            fee_routing_k_nad,
        }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        if self.fee_credit == 0 {
            return Ok(FeeLedgerReceipt::from_side(market_side));
        }
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.protocol_fee_bps,
            ErrorCode::InvalidMarketConfig
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps
                .checked_add(self.protocol_fee_bps)
                .ok_or(ErrorCode::InvalidMarketConfig)?,
            ErrorCode::InvalidMarketConfig
        );
        require!(self.fee_routing_k_nad > 0, ErrorCode::InvalidMarketConfig);

        let operator_fee = (self.fee_credit as u128)
            .checked_mul(self.operator_fee_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let operator_fee =
            u64::try_from(operator_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
        let protocol_fee = (self.fee_credit as u128)
            .checked_mul(self.protocol_fee_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let protocol_fee =
            u64::try_from(protocol_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
        let lp_fee = self
            .fee_credit
            .checked_sub(operator_fee)
            .and_then(|value| value.checked_sub(protocol_fee))
            .ok_or(ErrorCode::MarketMathOverflow)?;

        market_side.fee_ledger.fee_vault_balance = market_side
            .fee_ledger
            .fee_vault_balance
            .checked_add(self.fee_credit)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fee_ledger.operator_fee_liability = market_side
            .fee_ledger
            .operator_fee_liability
            .checked_add(operator_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fee_ledger.protocol_fee_liability = market_side
            .fee_ledger
            .protocol_fee_liability
            .checked_add(protocol_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;

        let (free_lp_fee, routed_fee) = routed_lp_fee(market_side, lp_fee, self.fee_routing_k_nad)?;
        record_hedged_fee_credit(market_side, routed_fee)?;

        let active_units = active_stake_units(
            market_side.claim_token_ledger.staked_claim_token_supply,
            market_side.buffer_ledger.staked_buffer_share_amount,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        if free_lp_fee > 0 {
            market_side.fee_ledger.unallocated_fee_liability = market_side
                .fee_ledger
                .unallocated_fee_liability
                .checked_add(free_lp_fee)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }

        carry_forward_unallocated_fee_with_units(market_side, active_units)?;
        market_side.fee_ledger.assert_backed()?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

impl CarryForwardStakerFees {
    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        let active_units = active_stake_units(
            market_side.claim_token_ledger.staked_claim_token_supply,
            market_side.buffer_ledger.staked_buffer_share_amount,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        carry_forward_unallocated_fee_with_units(market_side, active_units)?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

impl CarryForwardHedgedFees {
    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeLedgerReceipt> {
        carry_forward_unallocated_hedged_fee_with_supply(
            market_side,
            market_side.claim_token_ledger.hedged_claim_token_supply,
        )?;
        Ok(FeeLedgerReceipt::from_side(market_side))
    }
}

impl PrepareStakerFeeClaim {
    pub fn new(fee_vault_balance: u64) -> Self {
        Self { fee_vault_balance }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        stake_position: &mut StakePosition,
    ) -> Result<FeeClaimReceipt> {
        CarryForwardStakerFees.apply(market_side)?;
        stake_position.accrue_fees(
            market_side.fee_ledger.fee_growth_index_nad,
            market_side.buffer_ledger.buffer_ratio_bps,
        )?;
        let fee_amount = stake_position.accrued_fee_amount;
        require!(fee_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            market_side.fee_ledger.fee_liability,
            fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        require_gte!(
            self.fee_vault_balance,
            fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(FeeClaimReceipt {
            fee_amount,
            remaining_fee_liability: market_side.fee_ledger.fee_liability,
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

impl SettleStakerFeeClaim {
    pub fn new(fee_amount: u64, fee_vault_balance: u64) -> Self {
        Self {
            fee_amount,
            fee_vault_balance,
        }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        stake_position: &mut StakePosition,
    ) -> Result<FeeClaimReceipt> {
        require!(self.fee_amount > 0, ErrorCode::AmountZero);
        market_side.fee_ledger.fee_liability = market_side
            .fee_ledger
            .fee_liability
            .checked_sub(self.fee_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fee_ledger.fee_vault_balance = self.fee_vault_balance;
        stake_position.accrued_fee_amount = 0;
        market_side.fee_ledger.assert_backed()?;
        Ok(FeeClaimReceipt {
            fee_amount: self.fee_amount,
            remaining_fee_liability: market_side.fee_ledger.fee_liability,
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

impl PrepareHedgedFeeClaim {
    pub fn new(fee_vault_balance: u64) -> Self {
        Self { fee_vault_balance }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        hedge_position: &mut HedgePosition,
    ) -> Result<FeeClaimReceipt> {
        CarryForwardHedgedFees.apply(market_side)?;
        hedge_position.accrue_fees(market_side.fee_ledger.hedged_fee_growth_index_nad)?;
        let fee_amount = hedge_position.accrued_fee_amount;
        require!(fee_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            market_side.fee_ledger.hedged_fee_liability,
            fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        require_gte!(
            self.fee_vault_balance,
            fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(FeeClaimReceipt {
            fee_amount,
            remaining_fee_liability: market_side.fee_ledger.hedged_fee_liability,
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

impl SettleHedgedFeeClaim {
    pub fn new(fee_amount: u64, fee_vault_balance: u64) -> Self {
        Self {
            fee_amount,
            fee_vault_balance,
        }
    }

    pub fn apply(
        self,
        market_side: &mut MarketSide,
        hedge_position: &mut HedgePosition,
    ) -> Result<FeeClaimReceipt> {
        require!(self.fee_amount > 0, ErrorCode::AmountZero);
        market_side.fee_ledger.hedged_fee_liability = market_side
            .fee_ledger
            .hedged_fee_liability
            .checked_sub(self.fee_amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        market_side.fee_ledger.fee_vault_balance = self.fee_vault_balance;
        hedge_position.accrued_fee_amount = 0;
        market_side.fee_ledger.assert_backed()?;
        Ok(FeeClaimReceipt {
            fee_amount: self.fee_amount,
            remaining_fee_liability: market_side.fee_ledger.hedged_fee_liability,
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

impl PrepareMarketFeeClaim {
    pub fn new(claim_kind: MarketFeeClaimKind, fee_vault_balance: u64) -> Self {
        Self {
            claim_kind,
            fee_vault_balance,
        }
    }

    pub fn apply(self, market_side: &MarketSide) -> Result<FeeClaimReceipt> {
        let fee_amount = market_side.fee_ledger.market_fee_liability(self.claim_kind);
        require!(fee_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.fee_vault_balance,
            fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        Ok(FeeClaimReceipt {
            fee_amount,
            remaining_fee_liability: fee_amount,
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

impl SettleMarketFeeClaim {
    pub fn new(claim_kind: MarketFeeClaimKind, fee_amount: u64, fee_vault_balance: u64) -> Self {
        Self {
            claim_kind,
            fee_amount,
            fee_vault_balance,
        }
    }

    pub fn apply(self, market_side: &mut MarketSide) -> Result<FeeClaimReceipt> {
        require!(self.fee_amount > 0, ErrorCode::AmountZero);
        let claimed_amount = market_side
            .fee_ledger
            .claim_market_fee_liability(self.claim_kind)?;
        require_eq!(
            claimed_amount,
            self.fee_amount,
            ErrorCode::UnbackedFeeLiability
        );
        market_side.fee_ledger.fee_vault_balance = self.fee_vault_balance;
        market_side.fee_ledger.assert_backed()?;
        Ok(FeeClaimReceipt {
            fee_amount: self.fee_amount,
            remaining_fee_liability: market_side.fee_ledger.market_fee_liability(self.claim_kind),
            fee_vault_balance: self.fee_vault_balance,
        })
    }
}

fn carry_forward_unallocated_fee_with_units(
    market_side: &mut MarketSide,
    active_units: u64,
) -> Result<()> {
    if active_units == 0 || market_side.fee_ledger.unallocated_fee_liability == 0 {
        return Ok(());
    }

    let fee_amount = market_side.fee_ledger.unallocated_fee_liability;
    market_side.fee_ledger.unallocated_fee_liability = 0;
    let index_delta = (fee_amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(active_units as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = index_delta
        .checked_mul(active_units as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = u64::try_from(allocated_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let unallocated_fee = fee_amount
        .checked_sub(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    market_side.fee_ledger.fee_growth_index_nad = market_side
        .fee_ledger
        .fee_growth_index_nad
        .checked_add(index_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.fee_liability = market_side
        .fee_ledger
        .fee_liability
        .checked_add(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.unallocated_fee_liability = market_side
        .fee_ledger
        .unallocated_fee_liability
        .checked_add(unallocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

fn record_hedged_fee_credit(market_side: &mut MarketSide, fee_amount: u64) -> Result<()> {
    if fee_amount == 0 {
        return Ok(());
    }
    market_side.fee_ledger.unallocated_hedged_fee_liability = market_side
        .fee_ledger
        .unallocated_hedged_fee_liability
        .checked_add(fee_amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    carry_forward_unallocated_hedged_fee_with_supply(
        market_side,
        market_side.claim_token_ledger.hedged_claim_token_supply,
    )
}

fn carry_forward_unallocated_hedged_fee_with_supply(
    market_side: &mut MarketSide,
    hedged_claim_token_supply: u64,
) -> Result<()> {
    if hedged_claim_token_supply == 0
        || market_side.fee_ledger.unallocated_hedged_fee_liability == 0
    {
        return Ok(());
    }

    let fee_amount = market_side.fee_ledger.unallocated_hedged_fee_liability;
    market_side.fee_ledger.unallocated_hedged_fee_liability = 0;
    let index_delta = (fee_amount as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(hedged_claim_token_supply as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = index_delta
        .checked_mul(hedged_claim_token_supply as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let allocated_fee = u64::try_from(allocated_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let unallocated_fee = fee_amount
        .checked_sub(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;

    market_side.fee_ledger.hedged_fee_growth_index_nad = market_side
        .fee_ledger
        .hedged_fee_growth_index_nad
        .checked_add(index_delta)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.hedged_fee_liability = market_side
        .fee_ledger
        .hedged_fee_liability
        .checked_add(allocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    market_side.fee_ledger.unallocated_hedged_fee_liability = market_side
        .fee_ledger
        .unallocated_hedged_fee_liability
        .checked_add(unallocated_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(())
}

fn routed_lp_fee(
    market_side: &MarketSide,
    lp_fee: u64,
    fee_routing_k_nad: u64,
) -> Result<(u64, u64)> {
    if lp_fee == 0 || market_side.claim_token_ledger.hedged_claim_token_supply == 0 {
        return Ok((lp_fee, 0));
    }
    let free_buffer = market_side.free_buffer()?;
    if free_buffer == 0 {
        return Ok((lp_fee, 0));
    }
    let eta_nad = (market_side.claim_token_ledger.hedged_claim_token_supply as u128)
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(free_buffer as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let free_share_nad = dynamic_free_fee_share_nad(eta_nad, fee_routing_k_nad)?;
    let free_lp_fee = (lp_fee as u128)
        .checked_mul(free_share_nad)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let free_lp_fee = u64::try_from(free_lp_fee).map_err(|_| ErrorCode::MarketMathOverflow)?;
    let routed_fee = lp_fee
        .checked_sub(free_lp_fee)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok((free_lp_fee, routed_fee))
}

fn dynamic_free_fee_share_nad(eta_nad: u128, fee_routing_k_nad: u64) -> Result<u128> {
    require!(fee_routing_k_nad > 0, ErrorCode::InvalidMarketConfig);
    if eta_nad == 0 {
        return Ok(0);
    }
    let x_nad = eta_nad
        .checked_mul(fee_routing_k_nad as u128)
        .and_then(|value| value.checked_div(NAD as u128))
        .unwrap_or(i64::MAX as u128)
        .min(i64::MAX as u128) as i64;
    let hedged_share_nad = taylor_exp(-x_nad, NAD, TAYLOR_TERMS) as u128;
    Ok((NAD as u128).saturating_sub(hedged_share_nad))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BufferLedger, ClaimTokenLedger, FeeLedger};
    use proptest::prelude::*;

    fn market_side() -> MarketSide {
        MarketSide {
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn stake_position(accrued_fee_amount: u64) -> StakePosition {
        StakePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            available_buffer_share_amount: 0,
            staked_claim_token_amount: 800,
            staked_buffer_share_amount: 200,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount,
            bump: 1,
        }
    }

    fn hedge_position(accrued_fee_amount: u64) -> HedgePosition {
        HedgePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_token_amount: 500,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount,
            bump: 1,
        }
    }

    #[test]
    fn staker_fee_claim_settles_position_and_liability() {
        let mut market_side = market_side();
        market_side.fee_ledger = FeeLedger {
            fee_vault_balance: 100,
            fee_liability: 75,
            ..FeeLedger::default()
        };
        let mut stake_position = stake_position(75);

        let pending = PrepareStakerFeeClaim::new(100)
            .apply(&mut market_side, &mut stake_position)
            .unwrap();
        assert_eq!(pending.fee_amount, 75);

        let settled = SettleStakerFeeClaim::new(pending.fee_amount, 25)
            .apply(&mut market_side, &mut stake_position)
            .unwrap();

        assert_eq!(settled.remaining_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 25);
        assert_eq!(stake_position.accrued_fee_amount, 0);
    }

    #[test]
    fn hedged_fee_claim_settles_position_and_liability() {
        let mut market_side = market_side();
        market_side.claim_token_ledger = ClaimTokenLedger {
            hedged_claim_token_supply: 500,
            ..ClaimTokenLedger::default()
        };
        market_side.fee_ledger = FeeLedger {
            fee_vault_balance: 90,
            hedged_fee_liability: 60,
            ..FeeLedger::default()
        };
        let mut hedge_position = hedge_position(60);

        let pending = PrepareHedgedFeeClaim::new(90)
            .apply(&mut market_side, &mut hedge_position)
            .unwrap();
        assert_eq!(pending.fee_amount, 60);

        let settled = SettleHedgedFeeClaim::new(pending.fee_amount, 30)
            .apply(&mut market_side, &mut hedge_position)
            .unwrap();

        assert_eq!(settled.remaining_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.hedged_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 30);
        assert_eq!(hedge_position.accrued_fee_amount, 0);
    }

    #[test]
    fn market_fee_claim_settles_selected_liability() {
        let mut market_side = market_side();
        market_side.fee_ledger = FeeLedger {
            fee_vault_balance: 100,
            operator_fee_liability: 40,
            protocol_fee_liability: 20,
            ..FeeLedger::default()
        };

        let pending = PrepareMarketFeeClaim::new(MarketFeeClaimKind::Operator, 100)
            .apply(&market_side)
            .unwrap();
        assert_eq!(pending.fee_amount, 40);

        let settled =
            SettleMarketFeeClaim::new(MarketFeeClaimKind::Operator, pending.fee_amount, 60)
                .apply(&mut market_side)
                .unwrap();

        assert_eq!(settled.remaining_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.operator_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 20);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 60);
    }

    #[test]
    fn fee_credit_accrues_protocol_liability_before_lp_allocation() {
        let mut market_side = market_side();
        market_side.claim_token_ledger.staked_claim_token_supply = 8_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 2_000;

        RecordFeeCredit::new(1_000, 1_000, 2_000, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.operator_fee_liability, 100);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 200);
        assert_eq!(market_side.fee_ledger.fee_liability, 700);
        assert_eq!(market_side.fee_ledger.total_liability().unwrap(), 1_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn fee_credit_allocates_only_to_matched_stake() {
        let mut market_side = market_side();
        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 100_000;

        RecordFeeCredit::new(1_000, 1_000, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side.fee_ledger.operator_fee_liability, 100);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 900);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 1_800_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn fee_credit_routes_pressure_share_to_hedged_liability() {
        let mut market_side = market_side();
        market_side.reserve_ledger.live_reserve = 1_000_000;
        market_side.claim_token_ledger.protected_claim_token_supply = 800_000;
        market_side.claim_token_ledger.hedged_claim_token_supply = 200_000;
        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 200_000;

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side.fee_ledger.protocol_fee_liability, 0);
        assert!(market_side.fee_ledger.hedged_fee_liability > 0);
        assert!(market_side.fee_ledger.hedged_fee_growth_index_nad > 0);
        assert!(market_side.fee_ledger.fee_liability < 1_000);
        assert_eq!(
            market_side.fee_ledger.total_liability().unwrap(),
            market_side.fee_ledger.fee_vault_balance
        );
    }

    #[test]
    fn unstaked_claims_do_not_receive_fee_growth() {
        let mut market_side = market_side();
        market_side.claim_token_ledger.protected_claim_token_supply = 800_000;

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn unallocated_fees_carry_forward_to_next_active_stake() {
        let mut market_side = market_side();

        RecordFeeCredit::new(1_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();
        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1_000);

        market_side.claim_token_ledger.staked_claim_token_supply = 800_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 200_000;
        CarryForwardStakerFees.apply(&mut market_side).unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 1_000_000);
        assert_eq!(market_side.fee_ledger.fee_liability, 1_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn unallocated_fee_rounding_dust_stays_carried_forward() {
        let mut market_side = market_side();
        market_side.claim_token_ledger.staked_claim_token_supply = 1_600_000_000;
        market_side.buffer_ledger.staked_buffer_share_amount = 400_000_000;

        RecordFeeCredit::new(1, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    proptest! {
        #[test]
        fn fee_credit_remains_backed_and_does_not_reprice_claim_principal(
            fee_credit in 1_u64..1_000_000_000_u64,
            operator_fee_bps in 0_u16..4_000_u16,
            protocol_fee_bps in 0_u16..4_000_u16,
            stake_half in 1_u64..500_000_000_u64,
            protected_claim_token_supply in 1_u64..1_000_000_000_u64,
        ) {
            let mut market_side = market_side();
            market_side.buffer_ledger.buffer_ratio_bps = 5_000;
            market_side.claim_token_ledger.protected_claim_token_supply = protected_claim_token_supply;
            market_side.claim_token_ledger.staked_claim_token_supply = stake_half;
            market_side.buffer_ledger.staked_buffer_share_amount = stake_half;
            let protected_before = market_side.claim_token_ledger.protected_claim_token_supply;
            let reserve_before = market_side.reserve_ledger;

            RecordFeeCredit::new(fee_credit, operator_fee_bps, protocol_fee_bps, NAD)
                .apply(&mut market_side)
                .unwrap();

            prop_assert_eq!(market_side.fee_ledger.fee_vault_balance, fee_credit);
            prop_assert_eq!(market_side.fee_ledger.total_liability().unwrap(), fee_credit);
            market_side.fee_ledger.assert_backed().unwrap();
            prop_assert_eq!(market_side.claim_token_ledger.protected_claim_token_supply, protected_before);
            prop_assert_eq!(market_side.reserve_ledger.live_reserve, reserve_before.live_reserve);
            prop_assert_eq!(market_side.reserve_ledger.cash_reserve, reserve_before.cash_reserve);
        }

        #[test]
        fn no_stake_fee_credit_carries_lp_fees_without_growth(
            fee_credit in 1_u64..1_000_000_000_u64,
            operator_fee_bps in 0_u16..2_000_u16,
            protocol_fee_bps in 0_u16..2_000_u16,
        ) {
            let mut market_side = market_side();

            RecordFeeCredit::new(fee_credit, operator_fee_bps, protocol_fee_bps, NAD)
                .apply(&mut market_side)
                .unwrap();

            prop_assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
            prop_assert_eq!(market_side.fee_ledger.fee_liability, 0);
            prop_assert_eq!(market_side.fee_ledger.total_liability().unwrap(), fee_credit);
            market_side.fee_ledger.assert_backed().unwrap();
        }
    }
}
