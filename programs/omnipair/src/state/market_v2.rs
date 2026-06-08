use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::ErrorCode;
use crate::utils::market_v2_math::{
    accrue_fee_liability, active_stake_units, required_buffer_for_claims, split_claim_minus_buffer,
};
use crate::utils::math::{ceil_div, slots_to_ms, taylor_exp, SqrtU128};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketConfigV2 {
    pub swap_fee_bps: u16,
    pub operator_fee_bps: u16,
    pub buffer_ratio_bps: u16,
    pub fee_routing_k_nad: u64,
    pub ema_half_life_ms: u64,
    pub directional_ema_half_life_ms: u64,
    pub k_ema_half_life_ms: u64,
    pub max_daily_borrow_bps: u16,
    pub max_daily_withdraw_bps: u16,
    pub spot_ema_divergence_bps: u16,
    pub recognized_collateral_cap_bps: u16,
    pub market_health_min_bps: u16,
    pub effective_debt_weight_min_bps: u16,
    pub effective_debt_gamma_nad: u64,
    pub soft_borrow_enabled: bool,
    pub hedged_lp_enabled: bool,
    pub start_time: i64,
}

impl MarketConfigV2 {
    pub fn validate(&self) -> Result<()> {
        require_gte!(
            BPS_DENOMINATOR,
            self.swap_fee_bps,
            ErrorCode::InvalidSwapFeeBps
        );
        require_gte!(
            BPS_DENOMINATOR,
            self.operator_fee_bps,
            ErrorCode::InvalidMarketConfigV2
        );
        require!(
            self.buffer_ratio_bps > 0 && self.buffer_ratio_bps < BPS_DENOMINATOR,
            ErrorCode::InvalidMarketBufferRatioV2
        );
        require!(
            self.max_daily_borrow_bps <= BPS_DENOMINATOR
                && self.max_daily_withdraw_bps <= BPS_DENOMINATOR
                && self.spot_ema_divergence_bps <= BPS_DENOMINATOR,
            ErrorCode::InvalidMarketConfigV2
        );
        require!(
            self.recognized_collateral_cap_bps >= BPS_DENOMINATOR
                && self.market_health_min_bps >= BPS_DENOMINATOR
                && self.effective_debt_weight_min_bps <= BPS_DENOMINATOR,
            ErrorCode::InvalidMarketConfigV2
        );
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ReserveLedgerV2 {
    pub live_reserve: u64,
    pub cash_reserve: u64,
    pub reserved_liability: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ClaimLedgerV2 {
    pub protected_claim_supply: u64,
    pub hedged_claim_supply: u64,
    pub staked_claim_supply: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct BufferBookV2 {
    pub buffer_shares: u64,
    pub staked_buffer_shares: u64,
    pub required_buffer: u64,
    pub buffer_ratio_bps: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct FeeLedgerV2 {
    pub fee_growth_index_nad: u128,
    pub fee_vault_balance: u64,
    pub fee_liability: u64,
    pub unallocated_fee_liability: u64,
    pub protocol_fee_liability: u64,
    pub operator_fee_liability: u64,
}

impl FeeLedgerV2 {
    pub fn total_liability(&self) -> Result<u64> {
        self.fee_liability
            .checked_add(self.protocol_fee_liability)
            .and_then(|value| value.checked_add(self.operator_fee_liability))
            .and_then(|value| value.checked_add(self.unallocated_fee_liability))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn assert_backed(&self) -> Result<()> {
        require_gte!(
            self.fee_vault_balance,
            self.total_liability()?,
            ErrorCode::UnbackedFeeLiabilityV2
        );
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DailyLimitBookV2 {
    pub borrowed_bucket: u64,
    pub withdrawn_bucket: u64,
    pub last_decay_slot: u64,
}

impl DailyLimitBookV2 {
    pub fn decay_to_slot(&mut self, current_slot: u64) -> Result<()> {
        self.borrowed_bucket =
            decayed_daily_bucket(self.borrowed_bucket, self.last_decay_slot, current_slot)?;
        self.withdrawn_bucket =
            decayed_daily_bucket(self.withdrawn_bucket, self.last_decay_slot, current_slot)?;
        self.last_decay_slot = current_slot;
        Ok(())
    }

    pub fn record_borrow(&mut self, amount: u64, limit: u64, current_slot: u64) -> Result<()> {
        self.decay_to_slot(current_slot)?;
        let next_bucket = self
            .borrowed_bucket
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require_gte!(limit, next_bucket, ErrorCode::DailyLimitExceededV2);
        self.borrowed_bucket = next_bucket;
        Ok(())
    }

    pub fn record_withdraw(&mut self, amount: u64, limit: u64, current_slot: u64) -> Result<()> {
        self.decay_to_slot(current_slot)?;
        let next_bucket = self
            .withdrawn_bucket
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require_gte!(limit, next_bucket, ErrorCode::DailyLimitExceededV2);
        self.withdrawn_bucket = next_bucket;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSideV2 {
    pub asset_mint: Pubkey,
    pub asset_decimals: u8,
    pub claim_mint: Pubkey,
    pub hedge_mint: Pubkey,
    pub hedge_vault: Pubkey,
    pub reserve_vault: Pubkey,
    pub collateral_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub stake_vault: Pubkey,
    pub reserve_ledger: ReserveLedgerV2,
    pub claim_ledger: ClaimLedgerV2,
    pub buffer_book: BufferBookV2,
    pub fee_ledger: FeeLedgerV2,
    pub daily_limit_book: DailyLimitBookV2,
}

impl MarketSideV2 {
    pub fn claim_floor(&self) -> Result<u64> {
        self.claim_ledger
            .protected_claim_supply
            .checked_add(self.buffer_book.required_buffer)
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn free_buffer(&self) -> Result<u64> {
        self.reserve_ledger
            .live_reserve
            .checked_sub(self.claim_ledger.protected_claim_supply)
            .ok_or(ErrorCode::InsufficientMarketClaimCoverageV2.into())
    }

    pub fn assert_claim_coverage(&self) -> Result<()> {
        require_gte!(
            self.reserve_ledger.live_reserve,
            self.claim_floor()?,
            ErrorCode::InsufficientMarketClaimCoverageV2
        );
        Ok(())
    }

    pub fn required_buffer_for_ratio(&self, buffer_ratio_bps: u16) -> Result<u64> {
        required_buffer_for_claims(self.claim_ledger.protected_claim_supply, buffer_ratio_bps)
    }

    pub fn assert_buffer_floor_for_ratio(&self, buffer_ratio_bps: u16) -> Result<u64> {
        let required_buffer = self.required_buffer_for_ratio(buffer_ratio_bps)?;
        require_gte!(
            self.buffer_book.buffer_shares,
            required_buffer,
            ErrorCode::InsufficientBufferSharesV2
        );
        require_gte!(
            self.reserve_ledger.live_reserve,
            self.claim_ledger
                .protected_claim_supply
                .checked_add(required_buffer)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
            ErrorCode::InsufficientMarketClaimCoverageV2
        );
        Ok(required_buffer)
    }

    pub fn apply_buffer_ratio(&mut self, buffer_ratio_bps: u16, required_buffer: u64) {
        self.buffer_book.buffer_ratio_bps = buffer_ratio_bps;
        self.buffer_book.required_buffer = required_buffer;
    }

    pub fn apply_reserve_deposit(&mut self, reserve_credit: u64) -> Result<(u64, u64)> {
        require!(reserve_credit > 0, ErrorCode::AmountZero);
        let (claim_amount, buffer_amount) =
            split_claim_minus_buffer(reserve_credit, self.buffer_book.buffer_ratio_bps)?;
        require!(claim_amount > 0 && buffer_amount > 0, ErrorCode::AmountZero);

        let next_claim_supply = self
            .claim_ledger
            .protected_claim_supply
            .checked_add(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let next_buffer_shares = self
            .buffer_book
            .buffer_shares
            .checked_add(buffer_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let next_required_buffer =
            required_buffer_for_claims(next_claim_supply, self.buffer_book.buffer_ratio_bps)?;
        require_gte!(
            next_buffer_shares,
            next_required_buffer,
            ErrorCode::InsufficientBufferSharesV2
        );

        self.reserve_ledger.live_reserve = self
            .reserve_ledger
            .live_reserve
            .checked_add(reserve_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        self.reserve_ledger.cash_reserve = self
            .reserve_ledger
            .cash_reserve
            .checked_add(reserve_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        self.claim_ledger.protected_claim_supply = next_claim_supply;
        self.buffer_book.buffer_shares = next_buffer_shares;
        self.buffer_book.required_buffer = next_required_buffer;
        self.assert_claim_coverage()?;

        Ok((claim_amount, buffer_amount))
    }

    pub fn apply_claim_redemption(&mut self, claim_amount: u64) -> Result<()> {
        require!(claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.claim_ledger.protected_claim_supply,
            claim_amount,
            ErrorCode::InsufficientMarketClaimCoverageV2
        );
        require_gte!(
            self.reserve_ledger.cash_reserve,
            claim_amount,
            ErrorCode::InsufficientMarketClaimCoverageV2
        );

        let next_claim_supply = self
            .claim_ledger
            .protected_claim_supply
            .checked_sub(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let next_required_buffer =
            required_buffer_for_claims(next_claim_supply, self.buffer_book.buffer_ratio_bps)?;
        let next_live_reserve = self
            .reserve_ledger
            .live_reserve
            .checked_sub(claim_amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        let reserve_floor = next_claim_supply
            .checked_add(next_required_buffer)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require_gte!(
            next_live_reserve,
            reserve_floor,
            ErrorCode::InsufficientMarketClaimCoverageV2
        );

        self.reserve_ledger.live_reserve = next_live_reserve;
        self.reserve_ledger.cash_reserve = self
            .reserve_ledger
            .cash_reserve
            .checked_sub(claim_amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;
        self.claim_ledger.protected_claim_supply = next_claim_supply;
        self.buffer_book.required_buffer = next_required_buffer;

        Ok(())
    }

    pub fn record_fee_credit(&mut self, fee_credit: u64, operator_fee_bps: u16) -> Result<()> {
        if fee_credit == 0 {
            return Ok(());
        }
        require_gte!(
            BPS_DENOMINATOR,
            operator_fee_bps,
            ErrorCode::InvalidMarketConfigV2
        );

        let operator_fee = (fee_credit as u128)
            .checked_mul(operator_fee_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let operator_fee =
            u64::try_from(operator_fee).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
        let lp_fee = fee_credit
            .checked_sub(operator_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;

        self.fee_ledger.fee_vault_balance = self
            .fee_ledger
            .fee_vault_balance
            .checked_add(fee_credit)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.fee_ledger.operator_fee_liability = self
            .fee_ledger
            .operator_fee_liability
            .checked_add(operator_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;

        let active_units = active_stake_units(
            self.claim_ledger.staked_claim_supply,
            self.buffer_book.staked_buffer_shares,
            self.buffer_book.buffer_ratio_bps,
        )?;
        if lp_fee == 0 {
            return Ok(());
        }
        if active_units == 0 {
            self.fee_ledger.unallocated_fee_liability = self
                .fee_ledger
                .unallocated_fee_liability
                .checked_add(lp_fee)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
            return Ok(());
        }

        let index_delta = (lp_fee as u128)
            .checked_mul(NAD as u128)
            .and_then(|value| value.checked_div(active_units as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let allocated_fee = index_delta
            .checked_mul(active_units as u128)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let allocated_fee =
            u64::try_from(allocated_fee).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
        let unallocated_fee = lp_fee
            .checked_sub(allocated_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;

        self.fee_ledger.fee_growth_index_nad = self
            .fee_ledger
            .fee_growth_index_nad
            .checked_add(index_delta)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.fee_ledger.fee_liability = self
            .fee_ledger
            .fee_liability
            .checked_add(allocated_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.fee_ledger.unallocated_fee_liability = self
            .fee_ledger
            .unallocated_fee_liability
            .checked_add(unallocated_fee)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DebtBookV2 {
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub soft_debt0_shares: u128,
    pub soft_debt1_shares: u128,
    pub borrow_index0_nad: u128,
    pub borrow_index1_nad: u128,
    pub hedged_debt0_nad: u128,
    pub hedged_debt1_nad: u128,
}

impl DebtBookV2 {
    pub fn debt_to_shares(amount: u64, borrow_index_nad: u128) -> Result<u128> {
        require!(amount > 0, ErrorCode::AmountZero);
        ceil_div(
            (amount as u128)
                .checked_mul(NAD as u128)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
            borrow_index_nad,
        )
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn shares_to_debt(shares: u128, borrow_index_nad: u128) -> Result<u128> {
        shares
            .checked_mul(borrow_index_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn fixed_debt0(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_debt0_shares, self.borrow_index0_nad)
    }

    pub fn fixed_debt1(&self) -> Result<u128> {
        Self::shares_to_debt(self.fixed_debt1_shares, self.borrow_index1_nad)
    }

    pub fn soft_debt0(&self) -> Result<u128> {
        self.soft_debt0_shares
            .checked_mul(self.borrow_index0_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn soft_debt1(&self) -> Result<u128> {
        self.soft_debt1_shares
            .checked_mul(self.borrow_index1_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn total_debt0(&self) -> Result<u128> {
        self.fixed_debt0()?
            .checked_add(self.soft_debt0()?)
            .and_then(|value| value.checked_add(self.hedged_debt0_nad))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn total_debt1(&self) -> Result<u128> {
        self.fixed_debt1()?
            .checked_add(self.soft_debt1()?)
            .and_then(|value| value.checked_add(self.hedged_debt1_nad))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RiskBookV2 {
    pub price0_ema_nad: u64,
    pub price1_ema_nad: u64,
    pub directional_price0_ema_nad: u64,
    pub directional_price1_ema_nad: u64,
    pub k_ema: u128,
    pub liquidity_ema: u128,
    pub liquidity0_ema: u128,
    pub liquidity1_ema: u128,
    pub last_snapshot_slot: u64,
}

impl RiskBookV2 {
    pub fn refreshed(
        &self,
        side0: &MarketSideV2,
        side1: &MarketSideV2,
        config: &MarketConfigV2,
        current_slot: u64,
    ) -> Result<Self> {
        let spot_price0_nad = market_spot_price_nad(side0, side1)?;
        let spot_price1_nad = market_spot_price_nad(side1, side0)?;
        let liquidity0 = normalize_to_nad(
            side0.reserve_ledger.live_reserve as u128,
            side0.asset_decimals,
        )?;
        let liquidity1 = normalize_to_nad(
            side1.reserve_ledger.live_reserve as u128,
            side1.asset_decimals,
        )?;
        let liquidity = market_liquidity_nad(side0, side1)?;
        let k = market_k_nad(side0, side1)?;

        let price0_ema_nad = ema_u64(
            self.price0_ema_nad,
            spot_price0_nad,
            self.last_snapshot_slot,
            current_slot,
            config.ema_half_life_ms,
        );
        let price1_ema_nad = ema_u64(
            self.price1_ema_nad,
            spot_price1_nad,
            self.last_snapshot_slot,
            current_slot,
            config.ema_half_life_ms,
        );
        let directional_price0_ema_nad = directional_ema_u64(
            self.directional_price0_ema_nad,
            spot_price0_nad,
            self.last_snapshot_slot,
            current_slot,
            config.directional_ema_half_life_ms,
        );
        let directional_price1_ema_nad = directional_ema_u64(
            self.directional_price1_ema_nad,
            spot_price1_nad,
            self.last_snapshot_slot,
            current_slot,
            config.directional_ema_half_life_ms,
        );
        let liquidity_ema = ema_u128(
            self.liquidity_ema,
            liquidity,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let k_ema = ema_u128(
            self.k_ema,
            k,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let liquidity0_ema = ema_u128(
            self.liquidity0_ema,
            liquidity0,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );
        let liquidity1_ema = ema_u128(
            self.liquidity1_ema,
            liquidity1,
            self.last_snapshot_slot,
            current_slot,
            config.k_ema_half_life_ms,
        );

        Ok(Self {
            price0_ema_nad,
            price1_ema_nad,
            directional_price0_ema_nad,
            directional_price1_ema_nad,
            k_ema,
            liquidity_ema,
            liquidity0_ema,
            liquidity1_ema,
            last_snapshot_slot: current_slot,
        })
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketHealthV2 {
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub effective_debt0_nad: u128,
    pub effective_debt1_nad: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct InsuranceReserveV2 {
    pub vault0: Pubkey,
    pub vault1: Pubkey,
    pub available0: u64,
    pub available1: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct RecognitionLedgerV2 {
    pub debt_bearing_collateral0_for_debt1: u64,
    pub debt_bearing_collateral1_for_debt0: u64,
    pub last_recognition_slot: u64,
}

#[account]
#[derive(InitSpace)]
pub struct MarginPositionV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub collateral0: u64,
    pub collateral1: u64,
    pub recognized_collateral0_for_debt1: u64,
    pub recognized_collateral1_for_debt0: u64,
    pub fixed_debt0_shares: u128,
    pub fixed_debt1_shares: u128,
    pub bump: u8,
}

impl MarginPositionV2 {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default() && self.market != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidMarginPositionV2);
        require_keys_eq!(self.market, market, ErrorCode::InvalidMarginPositionV2);
        Ok(())
    }

    pub fn idle_collateral0(&self) -> Result<u64> {
        self.collateral0
            .checked_sub(self.recognized_collateral0_for_debt1)
            .ok_or(ErrorCode::InsufficientRecognizedCollateralV2.into())
    }

    pub fn idle_collateral1(&self) -> Result<u64> {
        self.collateral1
            .checked_sub(self.recognized_collateral1_for_debt0)
            .ok_or(ErrorCode::InsufficientRecognizedCollateralV2.into())
    }

    pub fn fixed_debt0(&self, debt_book: &DebtBookV2) -> Result<u128> {
        DebtBookV2::shares_to_debt(self.fixed_debt0_shares, debt_book.borrow_index0_nad)
    }

    pub fn fixed_debt1(&self, debt_book: &DebtBookV2) -> Result<u128> {
        DebtBookV2::shares_to_debt(self.fixed_debt1_shares, debt_book.borrow_index1_nad)
    }
}

#[account]
#[derive(InitSpace)]
pub struct StakePositionV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub available_buffer_shares: u64,
    pub staked_claim_amount: u64,
    pub staked_buffer_shares: u64,
    pub fee_growth_checkpoint_nad: u128,
    pub accrued_fee_amount: u64,
    pub bump: u8,
}

impl StakePositionV2 {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.asset_mint = asset_mint;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default()
            && self.market != Pubkey::default()
            && self.asset_mint != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidStakePositionV2);
        require_keys_eq!(self.market, market, ErrorCode::InvalidStakePositionV2);
        require_keys_eq!(
            self.asset_mint,
            asset_mint,
            ErrorCode::InvalidStakePositionV2
        );
        Ok(())
    }

    pub fn credit_buffer_shares(&mut self, amount: u64) -> Result<()> {
        self.available_buffer_shares = self
            .available_buffer_shares
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }

    pub fn active_stake_units(&self, buffer_ratio_bps: u16) -> Result<u64> {
        active_stake_units(
            self.staked_claim_amount,
            self.staked_buffer_shares,
            buffer_ratio_bps,
        )
    }

    pub fn accrue_fees(&mut self, fee_growth_index_nad: u128, buffer_ratio_bps: u16) -> Result<()> {
        let active_units = self.active_stake_units(buffer_ratio_bps)?;
        let accrued_amount = accrue_fee_liability(
            active_units,
            fee_growth_index_nad,
            self.fee_growth_checkpoint_nad,
        )?;
        self.accrued_fee_amount = self
            .accrued_fee_amount
            .checked_add(accrued_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.fee_growth_checkpoint_nad = fee_growth_index_nad;
        Ok(())
    }

    pub fn stake(&mut self, claim_amount: u64, buffer_shares: u64) -> Result<()> {
        require!(claim_amount > 0 && buffer_shares > 0, ErrorCode::AmountZero);
        require_gte!(
            self.available_buffer_shares,
            buffer_shares,
            ErrorCode::InsufficientBufferSharesV2
        );
        self.available_buffer_shares = self
            .available_buffer_shares
            .checked_sub(buffer_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.staked_claim_amount = self
            .staked_claim_amount
            .checked_add(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.staked_buffer_shares = self
            .staked_buffer_shares
            .checked_add(buffer_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }

    pub fn unstake(&mut self, claim_amount: u64, buffer_shares: u64) -> Result<()> {
        require!(claim_amount > 0 && buffer_shares > 0, ErrorCode::AmountZero);
        require_gte!(
            self.staked_claim_amount,
            claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            self.staked_buffer_shares,
            buffer_shares,
            ErrorCode::InsufficientBufferSharesV2
        );
        self.staked_claim_amount = self
            .staked_claim_amount
            .checked_sub(claim_amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.staked_buffer_shares = self
            .staked_buffer_shares
            .checked_sub(buffer_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        self.available_buffer_shares = self
            .available_buffer_shares
            .checked_add(buffer_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct HedgePositionV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub asset_mint: Pubkey,
    pub hedged_claim_amount: u64,
    pub bump: u8,
}

impl HedgePositionV2 {
    pub fn initialize(&mut self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey, bump: u8) {
        self.owner = owner;
        self.market = market;
        self.asset_mint = asset_mint;
        self.bump = bump;
    }

    pub fn is_initialized(&self) -> bool {
        self.owner != Pubkey::default()
            && self.market != Pubkey::default()
            && self.asset_mint != Pubkey::default()
    }

    pub fn assert_position(&self, owner: Pubkey, market: Pubkey, asset_mint: Pubkey) -> Result<()> {
        require_keys_eq!(self.owner, owner, ErrorCode::InvalidHedgePositionV2);
        require_keys_eq!(self.market, market, ErrorCode::InvalidHedgePositionV2);
        require_keys_eq!(
            self.asset_mint,
            asset_mint,
            ErrorCode::InvalidHedgePositionV2
        );
        Ok(())
    }

    pub fn increase(&mut self, amount: u64) -> Result<()> {
        self.hedged_claim_amount = self
            .hedged_claim_amount
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }

    pub fn decrease(&mut self, amount: u64) -> Result<()> {
        require_gte!(
            self.hedged_claim_amount,
            amount,
            ErrorCode::InvalidHedgePositionV2
        );
        self.hedged_claim_amount = self
            .hedged_claim_amount
            .checked_sub(amount)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct MarketV2 {
    pub version: u8,
    pub asset0_mint: Pubkey,
    pub asset1_mint: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub side0: MarketSideV2,
    pub side1: MarketSideV2,
    pub config: MarketConfigV2,
    pub debt_book: DebtBookV2,
    pub risk_book: RiskBookV2,
    pub health: MarketHealthV2,
    pub recognition_ledger: RecognitionLedgerV2,
    pub insurance_reserve: InsuranceReserveV2,
    pub params_hash: [u8; 32],
    pub last_update_slot: u64,
    pub reduce_only: bool,
    pub bump: u8,
}

impl MarketV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        asset0_mint: Pubkey,
        asset1_mint: Pubkey,
        operator: Pubkey,
        manager: Pubkey,
        side0: MarketSideV2,
        side1: MarketSideV2,
        config: MarketConfigV2,
        params_hash: [u8; 32],
        current_slot: u64,
        bump: u8,
    ) -> Result<Self> {
        config.validate()?;
        require_keys_neq!(asset0_mint, asset1_mint, ErrorCode::InvalidMint);
        require_keys_eq!(asset0_mint, side0.asset_mint, ErrorCode::InvalidMint);
        require_keys_eq!(asset1_mint, side1.asset_mint, ErrorCode::InvalidMint);

        Ok(Self {
            version: MARKET_V2_VERSION,
            asset0_mint,
            asset1_mint,
            operator,
            manager,
            side0,
            side1,
            config,
            debt_book: DebtBookV2 {
                borrow_index0_nad: NAD as u128,
                borrow_index1_nad: NAD as u128,
                ..DebtBookV2::default()
            },
            risk_book: RiskBookV2 {
                last_snapshot_slot: current_slot,
                ..RiskBookV2::default()
            },
            health: MarketHealthV2::default(),
            recognition_ledger: RecognitionLedgerV2 {
                last_recognition_slot: current_slot,
                ..RecognitionLedgerV2::default()
            },
            insurance_reserve: InsuranceReserveV2::default(),
            params_hash,
            last_update_slot: current_slot,
            reduce_only: false,
            bump,
        })
    }

    pub fn assert_live(&self) -> Result<()> {
        self.assert_started()?;
        require!(!self.reduce_only, ErrorCode::MarketReduceOnlyV2);
        Ok(())
    }

    pub fn assert_started(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= self.config.start_time, ErrorCode::MarketNotStartedV2);
        Ok(())
    }

    pub fn side(&self, market_side_index: u8) -> Result<&MarketSideV2> {
        match market_side_index {
            0 => Ok(&self.side0),
            1 => Ok(&self.side1),
            _ => err!(ErrorCode::InvalidMarketSideV2),
        }
    }

    pub fn side_mut(&mut self, market_side_index: u8) -> Result<&mut MarketSideV2> {
        match market_side_index {
            0 => Ok(&mut self.side0),
            1 => Ok(&mut self.side1),
            _ => err!(ErrorCode::InvalidMarketSideV2),
        }
    }

    pub fn swap_sides(&self, asset_in_is_asset0: bool) -> (&MarketSideV2, &MarketSideV2) {
        if asset_in_is_asset0 {
            (&self.side0, &self.side1)
        } else {
            (&self.side1, &self.side0)
        }
    }

    pub fn swap_sides_mut(
        &mut self,
        asset_in_is_asset0: bool,
    ) -> (&mut MarketSideV2, &mut MarketSideV2) {
        if asset_in_is_asset0 {
            (&mut self.side0, &mut self.side1)
        } else {
            (&mut self.side1, &mut self.side0)
        }
    }

    pub fn assert_market_invariants(&self) -> Result<()> {
        self.side0.assert_claim_coverage()?;
        self.side1.assert_claim_coverage()?;
        self.side0.fee_ledger.assert_backed()?;
        self.side1.fee_ledger.assert_backed()?;
        Ok(())
    }

    pub fn refresh_market_health(&mut self) -> Result<()> {
        self.refresh_risk_book()?;
        let effective_debt0_nad = self.effective_debt0_nad()?;
        let effective_debt1_nad = self.effective_debt1_nad()?;
        let collateral1_value_for_debt0_nad = self.collateral_value_for_debt0_nad(
            self.recognition_ledger.debt_bearing_collateral1_for_debt0,
        )?;
        let collateral0_value_for_debt1_nad = self.collateral_value_for_debt1_nad(
            self.recognition_ledger.debt_bearing_collateral0_for_debt1,
        )?;
        let health0_bps = health_bps(collateral1_value_for_debt0_nad, effective_debt0_nad)?;
        let health1_bps = health_bps(collateral0_value_for_debt1_nad, effective_debt1_nad)?;
        self.health = MarketHealthV2 {
            recognized_collateral0_for_debt1: self
                .recognition_ledger
                .debt_bearing_collateral0_for_debt1,
            recognized_collateral1_for_debt0: self
                .recognition_ledger
                .debt_bearing_collateral1_for_debt0,
            effective_debt0_nad,
            effective_debt1_nad,
            health0_bps,
            health1_bps,
        };
        Ok(())
    }

    pub fn current_risk_book(&self) -> Result<RiskBookV2> {
        let current_slot = Clock::get()
            .map(|clock| clock.slot)
            .unwrap_or(self.last_update_slot);
        self.risk_book
            .refreshed(&self.side0, &self.side1, &self.config, current_slot)
    }

    pub fn refresh_risk_book(&mut self) -> Result<()> {
        self.risk_book = self.current_risk_book()?;
        self.last_update_slot = self.risk_book.last_snapshot_slot;
        Ok(())
    }

    pub fn enforce_daily_borrow_limit(&mut self, market_side_index: u8, amount: u64) -> Result<()> {
        self.refresh_risk_book()?;
        let current_slot = self.risk_book.last_snapshot_slot;
        let limit =
            self.daily_limit_for_side(market_side_index, self.config.max_daily_borrow_bps)?;
        self.side_mut(market_side_index)?
            .daily_limit_book
            .record_borrow(amount, limit, current_slot)
    }

    pub fn enforce_daily_withdraw_limit(
        &mut self,
        market_side_index: u8,
        amount: u64,
    ) -> Result<()> {
        self.refresh_risk_book()?;
        let current_slot = self.risk_book.last_snapshot_slot;
        let limit =
            self.daily_limit_for_side(market_side_index, self.config.max_daily_withdraw_bps)?;
        self.side_mut(market_side_index)?
            .daily_limit_book
            .record_withdraw(amount, limit, current_slot)
    }

    pub fn assert_spot_ema_divergence(&self) -> Result<()> {
        assert_price_divergence(
            market_spot_price_nad(&self.side0, &self.side1)?,
            self.risk_book.price0_ema_nad,
            self.config.spot_ema_divergence_bps,
        )?;
        assert_price_divergence(
            market_spot_price_nad(&self.side1, &self.side0)?,
            self.risk_book.price1_ema_nad,
            self.config.spot_ema_divergence_bps,
        )
    }

    pub fn effective_debt0_nad(&self) -> Result<u128> {
        self.effective_debt_nad(true)
    }

    pub fn effective_debt1_nad(&self) -> Result<u128> {
        self.effective_debt_nad(false)
    }

    pub fn collateral_value_for_debt0_nad(&self, collateral1_amount: u64) -> Result<u128> {
        self.collateral_value_nad(false, collateral1_amount, &self.risk_book)
    }

    pub fn collateral_value_for_debt1_nad(&self, collateral0_amount: u64) -> Result<u128> {
        self.collateral_value_nad(true, collateral0_amount, &self.risk_book)
    }

    pub fn collateral_amount_for_debt_value(
        &self,
        debt_asset_is_asset0: bool,
        debt_amount: u64,
    ) -> Result<u64> {
        self.collateral_amount_for_debt_value_with_risk(
            debt_asset_is_asset0,
            debt_amount,
            &self.current_risk_book()?,
        )
    }

    pub fn position_health_bps(
        &self,
        margin_position: &MarginPositionV2,
        debt_asset_is_asset0: bool,
    ) -> Result<u64> {
        let risk_book = self.current_risk_book()?;
        if debt_asset_is_asset0 {
            health_bps(
                self.collateral_value_nad(
                    false,
                    margin_position.recognized_collateral1_for_debt0,
                    &risk_book,
                )?,
                normalize_to_nad(
                    margin_position.fixed_debt0(&self.debt_book)?,
                    self.side0.asset_decimals,
                )?,
            )
        } else {
            health_bps(
                self.collateral_value_nad(
                    true,
                    margin_position.recognized_collateral0_for_debt1,
                    &risk_book,
                )?,
                normalize_to_nad(
                    margin_position.fixed_debt1(&self.debt_book)?,
                    self.side1.asset_decimals,
                )?,
            )
        }
    }

    pub fn assert_position_health(
        &self,
        margin_position: &MarginPositionV2,
        debt_asset_is_asset0: bool,
        min_health_bps: u64,
    ) -> Result<()> {
        require_gte!(
            self.position_health_bps(margin_position, debt_asset_is_asset0)?,
            min_health_bps,
            ErrorCode::InsufficientMarketHealthV2
        );
        Ok(())
    }

    pub fn assert_recognition_cap(
        &self,
        margin_position: &MarginPositionV2,
        debt_asset_is_asset0: bool,
    ) -> Result<()> {
        let risk_book = self.current_risk_book()?;
        let cap_bps = self.config.recognized_collateral_cap_bps as u128;
        if debt_asset_is_asset0 {
            let recognized = self.collateral_value_nad(
                false,
                margin_position.recognized_collateral1_for_debt0,
                &risk_book,
            )?;
            let total =
                self.collateral_value_nad(false, margin_position.collateral1, &risk_book)?;
            require_gte!(
                total
                    .checked_mul(cap_bps)
                    .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
                recognized,
                ErrorCode::InsufficientRecognizedCollateralV2
            );
        } else {
            let recognized = self.collateral_value_nad(
                true,
                margin_position.recognized_collateral0_for_debt1,
                &risk_book,
            )?;
            let total = self.collateral_value_nad(true, margin_position.collateral0, &risk_book)?;
            require_gte!(
                total
                    .checked_mul(cap_bps)
                    .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
                recognized,
                ErrorCode::InsufficientRecognizedCollateralV2
            );
        }
        Ok(())
    }

    pub fn assert_market_health(&self) -> Result<()> {
        if self.health.effective_debt0_nad > 0 {
            require_gte!(
                self.health.health0_bps,
                self.config.market_health_min_bps as u64,
                ErrorCode::InsufficientMarketHealthV2
            );
        }
        if self.health.effective_debt1_nad > 0 {
            require_gte!(
                self.health.health1_bps,
                self.config.market_health_min_bps as u64,
                ErrorCode::InsufficientMarketHealthV2
            );
        }
        Ok(())
    }

    pub fn apply_buffer_ratio_update(&mut self, buffer_ratio_bps: u16) -> Result<()> {
        self.assert_buffer_ratio_change_unlocked(buffer_ratio_bps)?;
        let required_buffer0 = self.side0.assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        let required_buffer1 = self.side1.assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        self.side0
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer0);
        self.side1
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer1);
        Ok(())
    }

    fn assert_buffer_ratio_change_unlocked(&self, buffer_ratio_bps: u16) -> Result<()> {
        if buffer_ratio_bps == self.side0.buffer_book.buffer_ratio_bps
            && buffer_ratio_bps == self.side1.buffer_book.buffer_ratio_bps
        {
            return Ok(());
        }
        require!(
            self.side0.claim_ledger.staked_claim_supply == 0
                && self.side1.claim_ledger.staked_claim_supply == 0
                && self.side0.buffer_book.staked_buffer_shares == 0
                && self.side1.buffer_book.staked_buffer_shares == 0
                && self.side0.fee_ledger.fee_liability == 0
                && self.side1.fee_ledger.fee_liability == 0,
            ErrorCode::InvalidMarketConfigV2
        );
        Ok(())
    }
}

impl MarketV2 {
    fn effective_debt_nad(&self, debt_asset_is_asset0: bool) -> Result<u128> {
        let (fixed_debt, soft_debt, hedged_debt_nad, debt_side) = if debt_asset_is_asset0 {
            (
                self.debt_book.fixed_debt0()?,
                self.debt_book.soft_debt0()?,
                self.debt_book.hedged_debt0_nad,
                &self.side0,
            )
        } else {
            (
                self.debt_book.fixed_debt1()?,
                self.debt_book.soft_debt1()?,
                self.debt_book.hedged_debt1_nad,
                &self.side1,
            )
        };
        let fixed_debt_nad = normalize_to_nad(fixed_debt, debt_side.asset_decimals)?;
        let soft_debt_nad = normalize_to_nad(soft_debt, debt_side.asset_decimals)?;
        let hedged_debt_nad = effective_hedged_debt_nad(
            hedged_debt_nad,
            self.risk_book.liquidity_ema,
            self.config.effective_debt_weight_min_bps,
            self.config.effective_debt_gamma_nad,
        )?;

        fixed_debt_nad
            .checked_add(soft_debt_nad)
            .and_then(|value| value.checked_add(hedged_debt_nad))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    fn collateral_value_nad(
        &self,
        collateral_is_asset0: bool,
        collateral_amount: u64,
        risk_book: &RiskBookV2,
    ) -> Result<u128> {
        if collateral_amount == 0 {
            return Ok(0);
        }
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            if collateral_is_asset0 {
                (
                    &self.side0,
                    &self.side1,
                    risk_book.price0_ema_nad,
                    risk_book.directional_price0_ema_nad,
                )
            } else {
                (
                    &self.side1,
                    &self.side0,
                    risk_book.price1_ema_nad,
                    risk_book.directional_price1_ema_nad,
                )
            };
        let collateral_reserve = normalize_to_nad(
            collateral_side.reserve_ledger.live_reserve as u128,
            collateral_side.asset_decimals,
        )?;
        let debt_reserve = normalize_to_nad(
            debt_side.reserve_ledger.live_reserve as u128,
            debt_side.asset_decimals,
        )?;
        let collateral_amount =
            normalize_to_nad(collateral_amount as u128, collateral_side.asset_decimals)?;
        let (collateral_virtual_reserve, debt_virtual_reserve) =
            virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                price_ema_nad,
                directional_price_ema_nad,
            )?;
        constant_product_amount_out(
            collateral_virtual_reserve,
            debt_virtual_reserve,
            collateral_amount,
        )
    }

    fn collateral_amount_for_debt_value_with_risk(
        &self,
        debt_asset_is_asset0: bool,
        debt_amount: u64,
        risk_book: &RiskBookV2,
    ) -> Result<u64> {
        let debt_with_incentive = ceil_div(
            (debt_amount as u128)
                .checked_mul((BPS_DENOMINATOR + LIQUIDATION_INCENTIVE_BPS) as u128)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let (collateral_side, debt_side, price_ema_nad, directional_price_ema_nad) =
            if debt_asset_is_asset0 {
                (
                    &self.side1,
                    &self.side0,
                    risk_book.price1_ema_nad,
                    risk_book.directional_price1_ema_nad,
                )
            } else {
                (
                    &self.side0,
                    &self.side1,
                    risk_book.price0_ema_nad,
                    risk_book.directional_price0_ema_nad,
                )
            };
        let collateral_reserve = normalize_to_nad(
            collateral_side.reserve_ledger.live_reserve as u128,
            collateral_side.asset_decimals,
        )?;
        let debt_reserve = normalize_to_nad(
            debt_side.reserve_ledger.live_reserve as u128,
            debt_side.asset_decimals,
        )?;
        let debt_amount_nad = normalize_to_nad(debt_with_incentive, debt_side.asset_decimals)?;
        let (collateral_virtual_reserve, debt_virtual_reserve) =
            virtual_reserves_at_pessimistic_price(
                collateral_reserve,
                debt_reserve,
                price_ema_nad,
                directional_price_ema_nad,
            )?;
        let collateral_amount_nad = constant_product_amount_in(
            collateral_virtual_reserve,
            debt_virtual_reserve,
            debt_amount_nad,
        )?;
        denormalize_from_nad_ceil(collateral_amount_nad, collateral_side.asset_decimals)
    }

    fn daily_limit_for_side(&self, market_side_index: u8, limit_bps: u16) -> Result<u64> {
        let (liquidity_ema, asset_decimals) = match market_side_index {
            0 => (self.risk_book.liquidity0_ema, self.side0.asset_decimals),
            1 => (self.risk_book.liquidity1_ema, self.side1.asset_decimals),
            _ => return err!(ErrorCode::InvalidMarketSideV2),
        };
        require!(liquidity_ema > 0, ErrorCode::InsufficientLiquidity);
        let limit_nad = liquidity_ema
            .checked_mul(limit_bps as u128)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        denormalize_from_nad_floor(limit_nad, asset_decimals)
    }
}

fn health_bps(recognized_collateral_value_nad: u128, effective_debt_nad: u128) -> Result<u64> {
    if effective_debt_nad == 0 {
        return Ok(u64::MAX);
    }
    let health = recognized_collateral_value_nad
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(effective_debt_nad))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    u64::try_from(health).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn effective_hedged_debt_nad(
    hedged_debt_nad: u128,
    liquidity_ema: u128,
    min_weight_bps: u16,
    gamma_nad: u64,
) -> Result<u128> {
    if hedged_debt_nad == 0 {
        return Ok(0);
    }
    let min_weight = min_weight_bps as u128;
    let variable_weight = (BPS_DENOMINATOR as u128)
        .checked_sub(min_weight)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let gamma_pressure_nad = if liquidity_ema == 0 {
        NAD as u128
    } else {
        (gamma_nad as u128)
            .checked_mul(hedged_debt_nad)
            .and_then(|value| value.checked_div(liquidity_ema))
            .ok_or(ErrorCode::MarketMathOverflowV2)?
            .min(NAD as u128)
    };
    let weight_bps = min_weight
        .checked_add(
            variable_weight
                .checked_mul(gamma_pressure_nad)
                .and_then(|value| value.checked_div(NAD as u128))
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
        )
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    hedged_debt_nad
        .checked_mul(weight_bps)
        .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn market_spot_price_nad(collateral_side: &MarketSideV2, debt_side: &MarketSideV2) -> Result<u64> {
    let collateral_reserve = normalize_to_nad(
        collateral_side.reserve_ledger.live_reserve as u128,
        collateral_side.asset_decimals,
    )?;
    let debt_reserve = normalize_to_nad(
        debt_side.reserve_ledger.live_reserve as u128,
        debt_side.asset_decimals,
    )?;
    if collateral_reserve == 0 {
        return Ok(0);
    }
    let price = debt_reserve
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(collateral_reserve))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    u64::try_from(price).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn market_k_nad(side0: &MarketSideV2, side1: &MarketSideV2) -> Result<u128> {
    normalize_to_nad(
        side0.reserve_ledger.live_reserve as u128,
        side0.asset_decimals,
    )?
    .checked_mul(normalize_to_nad(
        side1.reserve_ledger.live_reserve as u128,
        side1.asset_decimals,
    )?)
    .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn market_liquidity_nad(side0: &MarketSideV2, side1: &MarketSideV2) -> Result<u128> {
    market_k_nad(side0, side1)?
        .sqrt()
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn normalize_to_nad(amount: u128, decimals: u8) -> Result<u128> {
    match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => Ok(amount),
        std::cmp::Ordering::Less => amount
            .checked_mul(
                10_u128
                    .checked_pow((NAD_DECIMALS - decimals) as u32)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
            .ok_or(ErrorCode::MarketMathOverflowV2.into()),
        std::cmp::Ordering::Greater => Ok(amount
            .checked_div(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
            .ok_or(ErrorCode::MarketMathOverflowV2)?),
    }
}

fn denormalize_from_nad_ceil(amount_nad: u128, decimals: u8) -> Result<u64> {
    let value = match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => amount_nad,
        std::cmp::Ordering::Less => ceil_div(
            amount_nad,
            10_u128
                .checked_pow((NAD_DECIMALS - decimals) as u32)
                .ok_or(ErrorCode::MarketMathOverflowV2)?,
        )
        .ok_or(ErrorCode::MarketMathOverflowV2)?,
        std::cmp::Ordering::Greater => amount_nad
            .checked_mul(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
    };
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn denormalize_from_nad_floor(amount_nad: u128, decimals: u8) -> Result<u64> {
    let value = match decimals.cmp(&NAD_DECIMALS) {
        std::cmp::Ordering::Equal => amount_nad,
        std::cmp::Ordering::Less => amount_nad
            .checked_div(
                10_u128
                    .checked_pow((NAD_DECIMALS - decimals) as u32)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
        std::cmp::Ordering::Greater => amount_nad
            .checked_mul(
                10_u128
                    .checked_pow((decimals - NAD_DECIMALS) as u32)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
    };
    u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn virtual_reserves_at_pessimistic_price(
    collateral_reserve: u128,
    debt_reserve: u128,
    collateral_ema_price_nad: u64,
    collateral_directional_ema_price_nad: u64,
) -> Result<(u128, u128)> {
    if collateral_reserve == 0 || debt_reserve == 0 {
        return err!(ErrorCode::InsufficientLiquidity);
    }
    let pessimistic_price =
        collateral_ema_price_nad.min(collateral_directional_ema_price_nad) as u128;
    require!(pessimistic_price > 0, ErrorCode::InvalidMarketConfigV2);
    let k = collateral_reserve
        .checked_mul(debt_reserve)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let collateral_squared = k
        .checked_mul(NAD as u128)
        .and_then(|value| value.checked_div(pessimistic_price))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let debt_squared = k
        .checked_mul(pessimistic_price)
        .and_then(|value| value.checked_div(NAD as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    Ok((
        collateral_squared
            .sqrt()
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
        debt_squared.sqrt().ok_or(ErrorCode::MarketMathOverflowV2)?,
    ))
}

fn constant_product_amount_out(
    reserve_in: u128,
    reserve_out: u128,
    amount_in: u128,
) -> Result<u128> {
    if amount_in == 0 {
        return Ok(0);
    }
    let denominator = reserve_in
        .checked_add(amount_in)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    amount_in
        .checked_mul(reserve_out)
        .and_then(|value| value.checked_div(denominator))
        .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn constant_product_amount_in(
    reserve_in: u128,
    reserve_out: u128,
    amount_out: u128,
) -> Result<u128> {
    require_gte!(reserve_out, amount_out, ErrorCode::InsufficientLiquidity);
    let denominator = reserve_out
        .checked_sub(amount_out)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    require!(denominator > 0, ErrorCode::InsufficientLiquidity);
    ceil_div(
        amount_out
            .checked_mul(reserve_in)
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
        denominator,
    )
    .ok_or(ErrorCode::MarketMathOverflowV2.into())
}

fn ema_u64(last_ema: u64, input: u64, last_slot: u64, current_slot: u64, half_life_ms: u64) -> u64 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    u64::try_from(ema_u128(
        last_ema as u128,
        input as u128,
        last_slot,
        current_slot,
        half_life_ms,
    ))
    .unwrap_or(u64::MAX)
}

fn directional_ema_u64(
    last_ema: u64,
    input: u64,
    last_slot: u64,
    current_slot: u64,
    half_life_ms: u64,
) -> u64 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    input.min(ema_u64(
        last_ema,
        input,
        last_slot,
        current_slot,
        half_life_ms,
    ))
}

fn ema_u128(
    last_ema: u128,
    input: u128,
    last_slot: u64,
    current_slot: u64,
    half_life_ms: u64,
) -> u128 {
    if last_ema == 0 || input == 0 {
        return input;
    }
    let Some(dt) = slots_to_ms(last_slot, current_slot) else {
        return last_ema;
    };
    if dt == 0 || half_life_ms == 0 {
        return last_ema;
    }
    let x = (dt as u128)
        .saturating_mul(NATURAL_LOG_OF_TWO_NAD as u128)
        .checked_div(half_life_ms as u128)
        .unwrap_or(u128::MAX)
        .min(i64::MAX as u128) as i64;
    let alpha = taylor_exp(-x, NAD, TAYLOR_TERMS) as u128;
    input
        .saturating_mul((NAD as u128).saturating_sub(alpha))
        .saturating_add(last_ema.saturating_mul(alpha))
        .checked_div(NAD as u128)
        .unwrap_or(last_ema)
}

fn decayed_daily_bucket(bucket: u64, last_slot: u64, current_slot: u64) -> Result<u64> {
    if bucket == 0 {
        return Ok(0);
    }
    let Some(elapsed_ms) = slots_to_ms(last_slot, current_slot) else {
        return Ok(bucket);
    };
    if elapsed_ms >= MS_PER_DAY {
        return Ok(0);
    }
    let remaining_ms = (MS_PER_DAY - elapsed_ms) as u128;
    let decayed = (bucket as u128)
        .checked_mul(remaining_ms)
        .and_then(|value| value.checked_div(MS_PER_DAY as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    u64::try_from(decayed).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn assert_price_divergence(
    spot_price_nad: u64,
    ema_price_nad: u64,
    max_divergence_bps: u16,
) -> Result<()> {
    require!(
        spot_price_nad > 0 && ema_price_nad > 0,
        ErrorCode::InsufficientLiquidity
    );
    let diff = spot_price_nad.abs_diff(ema_price_nad);
    let divergence_bps = (diff as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(ema_price_nad as u128))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    require_gte!(
        max_divergence_bps as u128,
        divergence_bps,
        ErrorCode::MarketRiskCircuitBreakerV2
    );
    Ok(())
}

#[macro_export]
macro_rules! generate_market_v2_seeds {
    ($market:expr) => {
        [
            MARKET_V2_SEED_PREFIX,
            $market.asset0_mint.as_ref(),
            $market.asset1_mint.as_ref(),
            $market.params_hash.as_ref(),
            &[$market.bump],
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_market_side(asset_mint: Pubkey, buffer_ratio_bps: u16) -> MarketSideV2 {
        MarketSideV2 {
            asset_mint,
            asset_decimals: 6,
            claim_mint: Pubkey::new_unique(),
            hedge_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            buffer_book: BufferBookV2 {
                buffer_ratio_bps,
                ..BufferBookV2::default()
            },
            ..MarketSideV2::default()
        }
    }

    fn test_market() -> MarketV2 {
        let asset0_mint = Pubkey::new_unique();
        let asset1_mint = Pubkey::new_unique();
        MarketV2::initialize(
            asset0_mint,
            asset1_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            test_market_side(asset0_mint, 2_000),
            test_market_side(asset1_mint, 2_000),
            MarketConfigV2 {
                swap_fee_bps: 30,
                operator_fee_bps: 1_000,
                buffer_ratio_bps: 2_000,
                fee_routing_k_nad: NAD,
                ema_half_life_ms: 60_000,
                directional_ema_half_life_ms: 60_000,
                k_ema_half_life_ms: 60_000,
                max_daily_borrow_bps: 2_000,
                max_daily_withdraw_bps: 2_000,
                spot_ema_divergence_bps: 1_000,
                recognized_collateral_cap_bps: 10_000,
                market_health_min_bps: 11_000,
                effective_debt_weight_min_bps: 10_000,
                effective_debt_gamma_nad: NAD,
                soft_borrow_enabled: false,
                hedged_lp_enabled: true,
                start_time: 0,
            },
            [7_u8; 32],
            42,
            254,
        )
        .unwrap()
    }

    fn stake_position() -> StakePositionV2 {
        StakePositionV2 {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            available_buffer_shares: 0,
            staked_claim_amount: 0,
            staked_buffer_shares: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    fn hedge_position() -> HedgePositionV2 {
        HedgePositionV2 {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_amount: 0,
            bump: 1,
        }
    }

    fn margin_position() -> MarginPositionV2 {
        MarginPositionV2 {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            collateral0: 0,
            collateral1: 0,
            recognized_collateral0_for_debt1: 0,
            recognized_collateral1_for_debt0: 0,
            fixed_debt0_shares: 0,
            fixed_debt1_shares: 0,
            bump: 1,
        }
    }

    #[test]
    fn reserve_deposit_mints_claim_minus_buffer() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);

        let (claim_amount, buffer_amount) = market_side.apply_reserve_deposit(1_000_000).unwrap();

        assert_eq!(claim_amount, 800_000);
        assert_eq!(buffer_amount, 200_000);
        assert_eq!(market_side.reserve_ledger.live_reserve, 1_000_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 1_000_000);
        assert_eq!(market_side.claim_ledger.protected_claim_supply, 800_000);
        assert_eq!(market_side.buffer_book.buffer_shares, 200_000);
        assert_eq!(market_side.buffer_book.required_buffer, 200_000);
        assert_eq!(market_side.claim_floor().unwrap(), 1_000_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn claim_redemption_is_fixed_one_to_one_principal() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.apply_reserve_deposit(1_000_000).unwrap();
        market_side.record_fee_credit(10_000, 0).unwrap();

        market_side.apply_claim_redemption(100_000).unwrap();

        assert_eq!(market_side.reserve_ledger.live_reserve, 900_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 900_000);
        assert_eq!(market_side.claim_ledger.protected_claim_supply, 700_000);
        assert_eq!(market_side.buffer_book.required_buffer, 175_000);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 10_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 10_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn fee_ledger_allocates_only_to_matched_stake() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.claim_ledger.staked_claim_supply = 800_000;
        market_side.buffer_book.staked_buffer_shares = 100_000;

        market_side.record_fee_credit(1_000, 1_000).unwrap();

        assert_eq!(market_side.fee_ledger.fee_vault_balance, 1_000);
        assert_eq!(market_side.fee_ledger.operator_fee_liability, 100);
        assert_eq!(market_side.fee_ledger.fee_liability, 900);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 0);
        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 1_800_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn unstaked_claims_do_not_receive_fee_growth() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        market_side.claim_ledger.protected_claim_supply = 800_000;

        market_side.record_fee_credit(1_000, 0).unwrap();

        assert_eq!(market_side.fee_ledger.fee_growth_index_nad, 0);
        assert_eq!(market_side.fee_ledger.fee_liability, 0);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 1_000);
        market_side.fee_ledger.assert_backed().unwrap();
    }

    #[test]
    fn stake_position_accrues_checkpointed_non_compounding_fees() {
        let mut position = stake_position();
        position.credit_buffer_shares(200_000).unwrap();
        position.stake(800_000, 200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 3 * NAD as u128);

        position.accrue_fees(3 * NAD as u128, 2_000).unwrap();
        assert_eq!(position.accrued_fee_amount, 2_000_000);
    }

    #[test]
    fn hedge_position_tracks_one_to_one_nav_without_stake_rights() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        let mut position = hedge_position();

        market_side.claim_ledger.hedged_claim_supply = market_side
            .claim_ledger
            .hedged_claim_supply
            .checked_add(500_000)
            .unwrap();
        position.increase(500_000).unwrap();

        assert_eq!(position.hedged_claim_amount, 500_000);
        assert_eq!(market_side.claim_ledger.hedged_claim_supply, 500_000);
        assert_eq!(market_side.claim_ledger.staked_claim_supply, 0);
        assert_eq!(market_side.buffer_book.staked_buffer_shares, 0);

        position.decrease(125_000).unwrap();
        market_side.claim_ledger.hedged_claim_supply = market_side
            .claim_ledger
            .hedged_claim_supply
            .checked_sub(125_000)
            .unwrap();
        assert_eq!(position.hedged_claim_amount, 375_000);
        assert_eq!(market_side.claim_ledger.hedged_claim_supply, 375_000);
    }

    #[test]
    fn market_health_uses_recognized_collateral_not_idle_inventory() {
        let mut market = test_market();
        market.side0.reserve_ledger.live_reserve = 1_000_000_000;
        market.side1.reserve_ledger.live_reserve = 1_000_000_000;
        market.debt_book.fixed_debt0_shares =
            DebtBookV2::debt_to_shares(1_000, NAD as u128).unwrap();

        market.refresh_market_health().unwrap();
        assert_eq!(market.health.health0_bps, 0);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealthV2)
        );

        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = 1_500;
        market.refresh_market_health().unwrap();
        assert!(market.health.health0_bps >= 14_900);
        market.assert_market_health().unwrap();
    }

    #[test]
    fn market_health_rejects_raw_unit_decimal_pump() {
        let asset0_mint = Pubkey::new_unique();
        let asset1_mint = Pubkey::new_unique();
        let mut side0 = test_market_side(asset0_mint, 2_000);
        let mut side1 = test_market_side(asset1_mint, 2_000);
        side0.asset_decimals = 6;
        side1.asset_decimals = 9;
        side0.reserve_ledger.live_reserve = 1_000_000_000;
        side1.reserve_ledger.live_reserve = 1_000_000_000_000_000;
        let mut market = MarketV2::initialize(
            asset0_mint,
            asset1_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            side0,
            side1,
            MarketConfigV2 {
                recognized_collateral_cap_bps: 10_000,
                ..test_market().config
            },
            [8_u8; 32],
            42,
            253,
        )
        .unwrap();
        market.debt_book.fixed_debt0_shares =
            DebtBookV2::debt_to_shares(900_000_000, NAD as u128).unwrap();
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = 1_000_000_000;
        market.refresh_market_health().unwrap();

        assert!(market.health.health0_bps < market.config.market_health_min_bps as u64);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealthV2)
        );
    }

    #[test]
    fn buffer_ratio_update_recomputes_required_floor() {
        let mut market = test_market();
        market.side0.apply_reserve_deposit(1_000_000).unwrap();
        market.side1.apply_reserve_deposit(2_000_000).unwrap();
        market.side0.buffer_book.buffer_shares += 100_000;
        market.side0.reserve_ledger.live_reserve += 100_000;
        market.side1.buffer_book.buffer_shares += 200_000;
        market.side1.reserve_ledger.live_reserve += 200_000;

        market.apply_buffer_ratio_update(2_500).unwrap();

        assert_eq!(market.side0.buffer_book.buffer_ratio_bps, 2_500);
        assert_eq!(market.side0.buffer_book.required_buffer, 266_667);
        assert_eq!(market.side1.buffer_book.required_buffer, 533_334);
    }

    #[test]
    fn buffer_ratio_update_rejects_uncovered_floor() {
        let mut market = test_market();
        market.side0.apply_reserve_deposit(1_000_000).unwrap();
        market.side1.apply_reserve_deposit(1_000_000).unwrap();

        let err = market.apply_buffer_ratio_update(2_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientBufferSharesV2));
        assert_eq!(market.side0.buffer_book.buffer_ratio_bps, 2_000);
        assert_eq!(market.side0.buffer_book.required_buffer, 200_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_active_stake() {
        let mut market = test_market();
        market.side0.apply_reserve_deposit(1_000_000).unwrap();
        market.side1.apply_reserve_deposit(1_000_000).unwrap();
        market.side0.claim_ledger.staked_claim_supply = 800_000;
        market.side0.buffer_book.staked_buffer_shares = 200_000;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfigV2));
        assert_eq!(market.side0.buffer_book.buffer_ratio_bps, 2_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_staker_fee_liability() {
        let mut market = test_market();
        market.side0.apply_reserve_deposit(1_000_000).unwrap();
        market.side1.apply_reserve_deposit(1_000_000).unwrap();
        market.side1.fee_ledger.fee_liability = 1;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfigV2));
        assert_eq!(market.side1.buffer_book.buffer_ratio_bps, 2_000);
    }

    #[test]
    fn daily_borrow_limit_uses_side_liquidity_ema() {
        let mut market = test_market();
        market.side0.reserve_ledger.live_reserve = 1_000_000;
        market.side1.reserve_ledger.live_reserve = 1_000_000;
        market.refresh_risk_book().unwrap();

        market.enforce_daily_borrow_limit(0, 200_000).unwrap();
        let err = market.enforce_daily_borrow_limit(0, 1).unwrap_err();

        assert_eq!(err, error!(ErrorCode::DailyLimitExceededV2));
        assert_eq!(market.side0.daily_limit_book.borrowed_bucket, 200_000);
    }

    #[test]
    fn daily_limit_bucket_decays_over_one_day() {
        let mut book = DailyLimitBookV2 {
            borrowed_bucket: 100_000,
            withdrawn_bucket: 50_000,
            last_decay_slot: 0,
        };
        let half_day_slots = MS_PER_DAY / TARGET_MS_PER_SLOT / 2;

        book.decay_to_slot(half_day_slots).unwrap();

        assert_eq!(book.borrowed_bucket, 50_000);
        assert_eq!(book.withdrawn_bucket, 25_000);
    }

    #[test]
    fn daily_limit_rejects_zero_liquidity() {
        let mut market = test_market();

        let err = market.enforce_daily_borrow_limit(0, 1).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientLiquidity));
    }

    #[test]
    fn circuit_breaker_rejects_spot_ema_divergence() {
        let mut market = test_market();
        market.side0.reserve_ledger.live_reserve = 1_000_000;
        market.side1.reserve_ledger.live_reserve = 2_000_000;
        market.risk_book.price0_ema_nad = NAD;
        market.risk_book.price1_ema_nad = NAD;

        let err = market.assert_spot_ema_divergence().unwrap_err();

        assert_eq!(err, error!(ErrorCode::MarketRiskCircuitBreakerV2));
    }

    #[test]
    fn effective_debt_applies_gamma_only_to_hedged_overlay() {
        let mut market = test_market();
        market.side0.reserve_ledger.live_reserve = 2_000_000_000;
        market.side1.reserve_ledger.live_reserve = 2_000_000_000;
        market.config.effective_debt_weight_min_bps = 5_000;
        market.config.effective_debt_gamma_nad = 2 * NAD;
        market.risk_book.liquidity_ema = 1_000 * NAD as u128;
        market.debt_book.fixed_debt0_shares =
            DebtBookV2::debt_to_shares(100_000_000, NAD as u128).unwrap();
        market.debt_book.hedged_debt0_nad = 100 * NAD as u128;

        let effective = market.effective_debt0_nad().unwrap();

        assert_eq!(effective, 160 * NAD as u128);
    }

    #[test]
    fn stale_recognition_cannot_exceed_margin_collateral() {
        let mut position = margin_position();
        position.collateral0 = 100;
        position.recognized_collateral0_for_debt1 = 80;
        assert_eq!(position.idle_collateral0().unwrap(), 20);

        position.recognized_collateral0_for_debt1 = 101;
        assert_eq!(
            position.idle_collateral0().unwrap_err(),
            error!(ErrorCode::InsufficientRecognizedCollateralV2)
        );
    }
}
