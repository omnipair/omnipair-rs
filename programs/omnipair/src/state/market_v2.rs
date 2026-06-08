use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::ErrorCode;
use crate::utils::market_v2_math::{required_buffer_for_claims, split_claim_minus_buffer};

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
    pub protocol_fee_liability: u64,
    pub operator_fee_liability: u64,
}

impl FeeLedgerV2 {
    pub fn total_liability(&self) -> Result<u64> {
        self.fee_liability
            .checked_add(self.protocol_fee_liability)
            .and_then(|value| value.checked_add(self.operator_fee_liability))
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSideV2 {
    pub asset_mint: Pubkey,
    pub claim_mint: Pubkey,
    pub hedge_mint: Pubkey,
    pub reserve_vault: Pubkey,
    pub fee_vault: Pubkey,
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
    pub fn fixed_debt0(&self) -> Result<u128> {
        self.fixed_debt0_shares
            .checked_mul(self.borrow_index0_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }

    pub fn fixed_debt1(&self) -> Result<u128> {
        self.fixed_debt1_shares
            .checked_mul(self.borrow_index1_nad)
            .and_then(|value| value.checked_div(NAD as u128))
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
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
    pub last_snapshot_slot: u64,
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
    pub accrued_fee_claim: u64,
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

    pub fn assert_market_invariants(&self) -> Result<()> {
        self.side0.assert_claim_coverage()?;
        self.side1.assert_claim_coverage()?;
        self.side0.fee_ledger.assert_backed()?;
        self.side1.fee_ledger.assert_backed()?;
        Ok(())
    }
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
