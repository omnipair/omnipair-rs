pub mod config;
pub mod debt;
pub mod fees;
pub mod health;
pub mod hlp;
pub mod insurance;
pub mod limits;
pub mod reserves;
pub mod risk;
pub mod shares;
pub mod side;
pub(crate) mod transitions;

pub use config::*;
pub use debt::*;
pub use fees::*;
pub use hlp::*;
pub use insurance::*;
pub use limits::*;
pub use reserves::*;
pub use risk::*;
pub use shares::*;
pub use side::*;

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::ErrorCode;
use crate::math::{
    accrued_index_nad, adapt_rate_at_target_nad, instantaneous_rate_apr_nad, utilization_bps,
    utilization_error_nad,
};
use crate::state::{
    futarchy_authority::{FutarchyAuthority, ProtocolAuctionSplit},
    margin_position::{CollateralReceipt, MarginPosition},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketTimelockAction {
    Scheduled { execute_after_slot: u64 },
    Ready,
}

pub struct AddLiquidityReceipt {
    pub base_reserve_credit: u64,
    pub quote_reserve_credit: u64,
    pub ylp_amount: u64,
    pub ylp_supply: u64,
}

pub struct RemoveLiquidityReceipt {
    pub ylp_amount: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub ylp_supply: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebtReceipt {
    pub debt_delta: i64,
    pub interest_paid: u64,
    pub fixed_base_debt: u128,
    pub fixed_quote_debt: u128,
    pub base_debt_health_bps: u64,
    pub quote_debt_health_bps: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwapReceipt {
    pub amount_in_after_fee: u64,
    pub amount_out: u64,
    pub fee_credit: u64,
    pub reserve_in_live_reserve: u64,
    pub reserve_out_live_reserve: u64,
    pub fees: FeesReceipt,
}

impl DebtReceipt {
    fn from_market(market: &Market, debt_delta: i64, interest_paid: u64) -> Result<Self> {
        Ok(Self {
            debt_delta,
            interest_paid,
            fixed_base_debt: market.debt.fixed_base_debt()?,
            fixed_quote_debt: market.debt.fixed_quote_debt()?,
            base_debt_health_bps: market.health.base_debt_health_bps,
            quote_debt_health_bps: market.health.quote_debt_health_bps,
        })
    }
}

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq, InitSpace,
)]
pub struct PendingAuthorityChange {
    pub active: bool,
    pub new_authority: Pubkey,
    pub scheduled_by: Pubkey,
    pub scheduled_slot: u64,
    pub execute_after_slot: u64,
}

impl PendingAuthorityChange {
    fn schedule(
        &mut self,
        new_authority: Pubkey,
        signer: Pubkey,
        current_slot: u64,
    ) -> Result<u64> {
        let execute_after_slot = current_slot
            .checked_add(MARKET_GOVERNANCE_DELAY_SLOTS)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.active = true;
        self.new_authority = new_authority;
        self.scheduled_by = signer;
        self.scheduled_slot = current_slot;
        self.execute_after_slot = execute_after_slot;
        Ok(execute_after_slot)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq, InitSpace,
)]
pub struct PendingConfigChange {
    pub active: bool,
    pub config: MarketConfig,
    pub scheduled_by: Pubkey,
    pub scheduled_slot: u64,
    pub execute_after_slot: u64,
}

impl PendingConfigChange {
    fn schedule(&mut self, config: MarketConfig, signer: Pubkey, current_slot: u64) -> Result<u64> {
        let execute_after_slot = current_slot
            .checked_add(MARKET_GOVERNANCE_DELAY_SLOTS)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        self.active = true;
        self.config = config;
        self.scheduled_by = signer;
        self.scheduled_slot = current_slot;
        self.execute_after_slot = execute_after_slot;
        Ok(execute_after_slot)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[account]
#[derive(InitSpace)]
pub struct Market {
    pub version: u8,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub ylp_mint: Pubkey,
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub base_side: MarketSide,
    pub quote_side: MarketSide,
    pub config: MarketConfig,
    pub debt: Debt,
    pub base_hlp_vault: HlpVault,
    pub quote_hlp_vault: HlpVault,
    pub risk: Risk,
    pub health: MarketHealth,
    pub insurance: Insurance,
    pub pending_config: PendingConfigChange,
    pub pending_operator: PendingAuthorityChange,
    pub pending_manager: PendingAuthorityChange,
    pub params_hash: [u8; 32],
    pub last_update_slot: u64,
    pub reduce_only: bool,
    pub bump: u8,
}

impl Market {
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        base_mint: Pubkey,
        quote_mint: Pubkey,
        ylp_mint: Pubkey,
        operator: Pubkey,
        manager: Pubkey,
        base_side: MarketSide,
        quote_side: MarketSide,
        config: MarketConfig,
        base_hlp_ylp_vault: Pubkey,
        quote_hlp_ylp_vault: Pubkey,
        params_hash: [u8; 32],
        current_slot: u64,
        bump: u8,
    ) -> Result<Self> {
        config.validate()?;
        require_keys_neq!(base_mint, quote_mint, ErrorCode::InvalidMint);
        require_keys_neq!(operator, Pubkey::default(), ErrorCode::InvalidMarketConfig);
        require_keys_neq!(manager, Pubkey::default(), ErrorCode::InvalidMarketConfig);
        require_keys_eq!(base_mint, base_side.asset_mint, ErrorCode::InvalidMint);
        require_keys_eq!(quote_mint, quote_side.asset_mint, ErrorCode::InvalidMint);

        Ok(Self {
            version: MARKET_VERSION,
            base_mint,
            quote_mint,
            ylp_mint,
            operator,
            manager,
            base_side,
            quote_side,
            config,
            debt: Debt {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                base_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                quote_rate_at_target_nad: INTEREST_INITIAL_RATE_AT_TARGET_NAD,
                last_recognition_slot: current_slot,
                last_accrual_slot: current_slot,
                ..Debt::default()
            },
            base_hlp_vault: {
                let mut vault = HlpVault::default();
                vault.initialize(MarketAsset::Base, base_hlp_ylp_vault, current_slot);
                vault
            },
            quote_hlp_vault: {
                let mut vault = HlpVault::default();
                vault.initialize(MarketAsset::Quote, quote_hlp_ylp_vault, current_slot);
                vault
            },
            risk: Risk {
                last_snapshot_slot: current_slot,
                ..Risk::default()
            },
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            pending_config: PendingConfigChange::default(),
            pending_operator: PendingAuthorityChange::default(),
            pending_manager: PendingAuthorityChange::default(),
            params_hash,
            last_update_slot: current_slot,
            reduce_only: false,
            bump,
        })
    }

    pub fn assert_live(&self) -> Result<()> {
        self.assert_started()?;
        require!(!self.reduce_only, ErrorCode::MarketReduceOnly);
        Ok(())
    }

    pub fn assert_live_with_futarchy(&self, futarchy_authority: &FutarchyAuthority) -> Result<()> {
        self.assert_started()?;
        require!(
            !futarchy_authority.is_reduce_only(self.reduce_only),
            ErrorCode::ReduceOnlyMode
        );
        Ok(())
    }

    pub fn assert_started(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= self.config.start_time, ErrorCode::MarketNotStarted);
        Ok(())
    }

    /// Accrue borrow interest up to the current slot. Should be called before any
    /// debt-dependent computation in an instruction (borrow/repay, hedge,
    /// liquidation, yield claims, swaps, and liquidity changes).
    pub fn accrue_interest(&mut self) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        self.accrue_interest_to_slot(current_slot)
    }

    pub fn update(&mut self) -> Result<()> {
        self.accrue_interest()?;
        if self.base_side.reserves.live_reserve > 0 && self.quote_side.reserves.live_reserve > 0 {
            self.refresh_market_health()?;
        }
        Ok(())
    }

    pub(crate) fn accrue_interest_to_slot(&mut self, current_slot: u64) -> Result<()> {
        let last = self.debt.last_accrual_slot;
        if current_slot <= last {
            return Ok(());
        }
        let dt_ms = current_slot
            .checked_sub(last)
            .ok_or(ErrorCode::MarketMathOverflow)?
            .saturating_mul(TARGET_MS_PER_SLOT);

        accrue_side(self, MarketAsset::Base, dt_ms)?;
        accrue_side(self, MarketAsset::Quote, dt_ms)?;
        self.debt.last_accrual_slot = current_slot;
        Ok(())
    }

    /// Manager-only authority: sensitive actions (fee setting, risk parameter
    /// changes, and role rotation) require the market manager.
    pub fn assert_manager(&self, signer: Pubkey) -> Result<()> {
        require_keys_eq!(signer, self.manager, ErrorCode::InvalidMarketManager);
        Ok(())
    }

    /// Config authority is manager-only. The operator remains the market's
    /// operational/economic identity, not a config admin.
    pub fn assert_config_authority(&self, signer: Pubkey) -> Result<()> {
        require_keys_eq!(
            signer,
            self.manager,
            ErrorCode::InvalidMarketConfigAuthority
        );
        Ok(())
    }

    pub fn prepare_config_update(
        &mut self,
        signer: Pubkey,
        config: MarketConfig,
        current_slot: u64,
    ) -> Result<MarketTimelockAction> {
        self.assert_config_authority(signer)?;
        config.validate()?;
        if self.pending_config.active && self.pending_config.config == config {
            require_gte!(
                current_slot,
                self.pending_config.execute_after_slot,
                ErrorCode::GovernanceTimelockNotReady
            );
            return Ok(MarketTimelockAction::Ready);
        }
        require!(config != self.config, ErrorCode::InvalidArgument);
        let execute_after_slot = self.pending_config.schedule(config, signer, current_slot)?;
        Ok(MarketTimelockAction::Scheduled { execute_after_slot })
    }

    pub fn clear_pending_config_update(&mut self) {
        self.pending_config.clear();
    }

    pub fn prepare_operator_update(
        &mut self,
        signer: Pubkey,
        new_operator: Pubkey,
        current_slot: u64,
    ) -> Result<MarketTimelockAction> {
        self.assert_manager(signer)?;
        require_keys_neq!(new_operator, Pubkey::default(), ErrorCode::InvalidArgument);
        require_keys_neq!(new_operator, self.operator, ErrorCode::InvalidArgument);
        if self.pending_operator.active && self.pending_operator.new_authority == new_operator {
            require_gte!(
                current_slot,
                self.pending_operator.execute_after_slot,
                ErrorCode::GovernanceTimelockNotReady
            );
            return Ok(MarketTimelockAction::Ready);
        }
        let execute_after_slot =
            self.pending_operator
                .schedule(new_operator, signer, current_slot)?;
        Ok(MarketTimelockAction::Scheduled { execute_after_slot })
    }

    pub fn apply_operator_update(&mut self, new_operator: Pubkey) {
        self.operator = new_operator;
        self.pending_operator.clear();
    }

    pub fn prepare_manager_update(
        &mut self,
        signer: Pubkey,
        new_manager: Pubkey,
        current_slot: u64,
    ) -> Result<MarketTimelockAction> {
        self.assert_manager(signer)?;
        require_keys_neq!(new_manager, Pubkey::default(), ErrorCode::InvalidArgument);
        require_keys_neq!(new_manager, self.manager, ErrorCode::InvalidArgument);
        if self.pending_manager.active && self.pending_manager.new_authority == new_manager {
            require_gte!(
                current_slot,
                self.pending_manager.execute_after_slot,
                ErrorCode::GovernanceTimelockNotReady
            );
            return Ok(MarketTimelockAction::Ready);
        }
        let execute_after_slot =
            self.pending_manager
                .schedule(new_manager, signer, current_slot)?;
        Ok(MarketTimelockAction::Scheduled { execute_after_slot })
    }

    pub fn apply_manager_update(&mut self, new_manager: Pubkey) {
        self.manager = new_manager;
        self.pending_manager.clear();
    }

    pub fn side(&self, market_asset: MarketAsset) -> Result<&MarketSide> {
        match market_asset {
            MarketAsset::Base => Ok(&self.base_side),
            MarketAsset::Quote => Ok(&self.quote_side),
        }
    }

    pub fn side_mut(&mut self, market_asset: MarketAsset) -> Result<&mut MarketSide> {
        match market_asset {
            MarketAsset::Base => Ok(&mut self.base_side),
            MarketAsset::Quote => Ok(&mut self.quote_side),
        }
    }

    pub fn asset_for_mint(&self, mint: Pubkey) -> Result<MarketAsset> {
        if mint == self.base_side.asset_mint {
            return Ok(MarketAsset::Base);
        }
        if mint == self.quote_side.asset_mint {
            return Ok(MarketAsset::Quote);
        }
        err!(ErrorCode::InvalidMint)
    }

    pub fn asset_for_hlp_mint(&self, mint: Pubkey) -> Result<MarketAsset> {
        if mint == self.base_side.hlp_mint {
            return Ok(MarketAsset::Base);
        }
        if mint == self.quote_side.hlp_mint {
            return Ok(MarketAsset::Quote);
        }
        err!(ErrorCode::InvalidLpMintKey)
    }

    pub fn swap_sides(&self, asset_in: MarketAsset) -> (&MarketSide, &MarketSide) {
        match asset_in {
            MarketAsset::Base => (&self.base_side, &self.quote_side),
            MarketAsset::Quote => (&self.quote_side, &self.base_side),
        }
    }

    pub fn swap_sides_mut(&mut self, asset_in: MarketAsset) -> (&mut MarketSide, &mut MarketSide) {
        match asset_in {
            MarketAsset::Base => (&mut self.base_side, &mut self.quote_side),
            MarketAsset::Quote => (&mut self.quote_side, &mut self.base_side),
        }
    }

    pub fn base_quote_sides_mut(&mut self) -> (&mut MarketSide, &mut MarketSide) {
        (&mut self.base_side, &mut self.quote_side)
    }

    pub fn withdraw_collateral(
        &mut self,
        margin_position: &mut MarginPosition,
        market_asset: MarketAsset,
        collateral_debit: u64,
    ) -> Result<CollateralReceipt> {
        require!(collateral_debit > 0, ErrorCode::AmountZero);
        self.enforce_daily_withdraw_limit(market_asset, collateral_debit)?;
        match market_asset {
            MarketAsset::Base => {
                require_gte!(
                    margin_position.idle_base_collateral()?,
                    collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.base_collateral = margin_position
                    .base_collateral
                    .checked_sub(collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                require_gte!(
                    margin_position.idle_quote_collateral()?,
                    collateral_debit,
                    ErrorCode::InsufficientRecognizedCollateral
                );
                margin_position.quote_collateral = margin_position
                    .quote_collateral
                    .checked_sub(collateral_debit)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        margin_position.record_risk_update()?;
        self.refresh_market_health()?;
        self.assert_risk_circuit_breakers()?;

        Ok(CollateralReceipt {
            collateral_credit: 0,
            collateral_debit,
            base_collateral: margin_position.base_collateral,
            quote_collateral: margin_position.quote_collateral,
        })
    }

    pub fn borrow(
        &mut self,
        margin_position: &mut MarginPosition,
        borrow_asset: MarketAsset,
        borrow_amount: u64,
        min_health_bps: u64,
    ) -> Result<DebtReceipt> {
        let debt_delta = i64::try_from(borrow_amount).map_err(|_| ErrorCode::Overflow)?;
        let debt_shares = match borrow_asset {
            MarketAsset::Base => {
                Debt::debt_to_shares(borrow_amount, self.debt.base_borrow_index_nad)?
            }
            MarketAsset::Quote => {
                Debt::debt_to_shares(borrow_amount, self.debt.quote_borrow_index_nad)?
            }
        };
        self.enforce_daily_borrow_limit(borrow_asset, borrow_amount)?;
        let debt_side = self.side_mut(borrow_asset)?;
        require_borrow_headroom(debt_side, borrow_amount)?;
        debt_side.reserves.live_reserve = debt_side
            .reserves
            .live_reserve
            .checked_sub(borrow_amount)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        debt_side.reserves.cash_reserve = debt_side
            .reserves
            .cash_reserve
            .checked_sub(borrow_amount)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        match borrow_asset {
            MarketAsset::Base => {
                margin_position.fixed_base_shares = margin_position
                    .fixed_base_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.fixed_base_shares = self
                    .debt
                    .fixed_base_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
            MarketAsset::Quote => {
                margin_position.fixed_quote_shares = margin_position
                    .fixed_quote_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.fixed_quote_shares = self
                    .debt
                    .fixed_quote_shares
                    .checked_add(debt_shares)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
            }
        }
        self.debt
            .add_margin_principal(borrow_asset, borrow_amount)?;
        sync_borrow_recognition(self, margin_position, borrow_asset)?;
        self.refresh_market_health()?;
        self.assert_market_health()?;
        self.assert_risk_circuit_breakers()?;
        self.assert_recognition_cap(margin_position, borrow_asset)?;
        self.assert_position_health(margin_position, borrow_asset, min_health_bps)?;
        let health = self.position_health_bps(margin_position, borrow_asset)?;
        require_gte!(health, min_health_bps, ErrorCode::InsufficientMarketHealth);
        margin_position.record_risk_update()?;
        DebtReceipt::from_market(self, debt_delta, 0)
    }

    pub fn repay(
        &mut self,
        margin_position: &mut MarginPosition,
        repay_asset: MarketAsset,
        repay_credit: u64,
    ) -> Result<DebtReceipt> {
        let debt_delta = -i64::try_from(repay_credit).map_err(|_| ErrorCode::Overflow)?;
        let debt_before = match repay_asset {
            MarketAsset::Base => margin_position.fixed_base_debt(&self.debt)?,
            MarketAsset::Quote => margin_position.fixed_quote_debt(&self.debt)?,
        };
        require_gte!(
            debt_before,
            repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let interest_paid = self.debt.realize_margin_repay(repay_asset, repay_credit)?;
        let principal_credit = repay_credit
            .checked_sub(interest_paid)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        match repay_asset {
            MarketAsset::Base => {
                let shares_before = margin_position.fixed_base_shares;
                let shares_to_burn = if repay_credit as u128 == debt_before {
                    shares_before
                } else {
                    Debt::debt_to_shares(repay_credit, self.debt.base_borrow_index_nad)?
                        .min(shares_before)
                };
                let release_collateral = proportional_release(
                    margin_position.recognized_quote_collateral_for_base_debt,
                    shares_to_burn,
                    shares_before,
                )?;
                margin_position.fixed_base_shares = margin_position
                    .fixed_base_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                margin_position.recognized_quote_collateral_for_base_debt = margin_position
                    .recognized_quote_collateral_for_base_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.fixed_base_shares = self
                    .debt
                    .fixed_base_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.recognized_quote_collateral_for_base_debt = self
                    .debt
                    .recognized_quote_collateral_for_base_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.base_side.reserves.live_reserve = self
                    .base_side
                    .reserves
                    .live_reserve
                    .checked_add(principal_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                self.base_side.reserves.cash_reserve = self
                    .base_side
                    .reserves
                    .cash_reserve
                    .checked_add(principal_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
            }
            MarketAsset::Quote => {
                let shares_before = margin_position.fixed_quote_shares;
                let shares_to_burn = if repay_credit as u128 == debt_before {
                    shares_before
                } else {
                    Debt::debt_to_shares(repay_credit, self.debt.quote_borrow_index_nad)?
                        .min(shares_before)
                };
                let release_collateral = proportional_release(
                    margin_position.recognized_base_collateral_for_quote_debt,
                    shares_to_burn,
                    shares_before,
                )?;
                margin_position.fixed_quote_shares = margin_position
                    .fixed_quote_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                margin_position.recognized_base_collateral_for_quote_debt = margin_position
                    .recognized_base_collateral_for_quote_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.fixed_quote_shares = self
                    .debt
                    .fixed_quote_shares
                    .checked_sub(shares_to_burn)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.debt.recognized_base_collateral_for_quote_debt = self
                    .debt
                    .recognized_base_collateral_for_quote_debt
                    .checked_sub(release_collateral)
                    .ok_or(ErrorCode::MarketMathOverflow)?;
                self.quote_side.reserves.live_reserve = self
                    .quote_side
                    .reserves
                    .live_reserve
                    .checked_add(principal_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                self.quote_side.reserves.cash_reserve = self
                    .quote_side
                    .reserves
                    .cash_reserve
                    .checked_add(principal_credit)
                    .ok_or(ErrorCode::ReserveOverflow)?;
            }
        }
        margin_position.record_risk_update()?;
        self.refresh_market_health()?;
        self.assert_risk_circuit_breakers()?;
        DebtReceipt::from_market(self, debt_delta, interest_paid)
    }

    pub fn add_liquidity(
        &mut self,
        base_reserve_credit: u64,
        quote_reserve_credit: u64,
    ) -> Result<AddLiquidityReceipt> {
        require!(
            base_reserve_credit > 0 && quote_reserve_credit > 0,
            ErrorCode::AmountZero
        );
        let base_reserve_before = self.base_side.reserves.live_reserve;
        let quote_reserve_before = self.quote_side.reserves.live_reserve;
        if base_reserve_before > 0 || quote_reserve_before > 0 {
            require!(
                base_reserve_before > 0 && quote_reserve_before > 0,
                ErrorCode::InsufficientLiquidity
            );
            let lhs = (base_reserve_credit as u128)
                .checked_mul(quote_reserve_before as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            let rhs = (quote_reserve_credit as u128)
                .checked_mul(base_reserve_before as u128)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            require_eq!(lhs, rhs, ErrorCode::SlippageExceeded);
        }

        let ylp_amount = self.ylp_for_deposit(
            base_reserve_before,
            quote_reserve_before,
            base_reserve_credit,
            quote_reserve_credit,
        )?;
        require!(ylp_amount > 0, ErrorCode::SlippageExceeded);

        self.base_side.credit_reserve(base_reserve_credit, true)?;
        self.quote_side.credit_reserve(quote_reserve_credit, true)?;
        self.base_side.shares.mint(ylp_amount)?;
        self.quote_side.shares.mint(ylp_amount)?;
        self.base_side.assert_share_backing()?;
        self.quote_side.assert_share_backing()?;

        Ok(AddLiquidityReceipt {
            base_reserve_credit,
            quote_reserve_credit,
            ylp_amount,
            ylp_supply: self.base_side.shares.ylp_supply,
        })
    }

    pub fn remove_liquidity(&mut self, ylp_amount: u64) -> Result<RemoveLiquidityReceipt> {
        require!(ylp_amount > 0, ErrorCode::AmountZero);
        require_eq!(
            self.base_side.shares.ylp_supply,
            self.quote_side.shares.ylp_supply,
            ErrorCode::BrokenInvariant
        );

        let base_amount_out = self
            .base_side
            .shares
            .reserve_for_burn(self.base_side.reserves.live_reserve, ylp_amount)?;
        let quote_amount_out = self
            .quote_side
            .shares
            .reserve_for_burn(self.quote_side.reserves.live_reserve, ylp_amount)?;
        require_gte!(
            self.base_side.reserves.cash_reserve,
            base_amount_out,
            ErrorCode::InsufficientLiquidity
        );
        require_gte!(
            self.quote_side.reserves.cash_reserve,
            quote_amount_out,
            ErrorCode::InsufficientLiquidity
        );

        self.base_side.debit_reserve(base_amount_out, true)?;
        self.quote_side.debit_reserve(quote_amount_out, true)?;
        self.base_side.shares.burn(ylp_amount)?;
        self.quote_side.shares.burn(ylp_amount)?;
        self.base_side.assert_share_backing()?;
        self.quote_side.assert_share_backing()?;

        Ok(RemoveLiquidityReceipt {
            ylp_amount,
            base_amount_out,
            quote_amount_out,
            ylp_supply: self.base_side.shares.ylp_supply,
        })
    }

    pub(crate) fn ylp_for_deposit(
        &self,
        base_reserve_before: u64,
        quote_reserve_before: u64,
        base_amount: u64,
        quote_amount: u64,
    ) -> Result<u64> {
        require_eq!(
            self.base_side.shares.ylp_supply,
            self.quote_side.shares.ylp_supply,
            ErrorCode::BrokenInvariant
        );
        if self.base_side.shares.ylp_supply == 0 {
            return Ok(base_amount);
        }
        let base_ylp = self
            .base_side
            .shares
            .shares_for_deposit(base_reserve_before, base_amount)?;
        let quote_ylp = self
            .quote_side
            .shares
            .shares_for_deposit(quote_reserve_before, quote_amount)?;
        Ok(base_ylp.min(quote_ylp))
    }

    pub fn swap_reserves(
        &mut self,
        asset_in: MarketAsset,
        amount_in_after_fee: u64,
        amount_out: u64,
        fee_credit: u64,
        manager_fee_bps: u16,
        protocol_fee_bps: u16,
        protocol_auction_split: ProtocolAuctionSplit,
    ) -> Result<SwapReceipt> {
        let (market_side_in, market_side_out) = self.swap_sides_mut(asset_in);
        require_gte!(
            market_side_out.reserves.cash_reserve,
            amount_out,
            ErrorCode::InsufficientLiquidity
        );

        market_side_in.reserves.live_reserve = market_side_in
            .reserves
            .live_reserve
            .checked_add(amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_in.reserves.cash_reserve = market_side_in
            .reserves
            .cash_reserve
            .checked_add(amount_in_after_fee)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market_side_out.reserves.live_reserve = market_side_out
            .reserves
            .live_reserve
            .checked_sub(amount_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        market_side_out.reserves.cash_reserve = market_side_out
            .reserves
            .cash_reserve
            .checked_sub(amount_out)
            .ok_or(ErrorCode::CashReserveUnderflow)?;

        let fees = market_side_in.record_swap_fee_credit(
            fee_credit,
            manager_fee_bps,
            protocol_fee_bps,
            protocol_auction_split,
        )?;
        market_side_in.assert_share_backing()?;
        market_side_out.assert_share_backing()?;
        market_side_in.fees.assert_backed()?;

        Ok(SwapReceipt {
            amount_in_after_fee,
            amount_out,
            fee_credit,
            reserve_in_live_reserve: market_side_in.reserves.live_reserve,
            reserve_out_live_reserve: market_side_out.reserves.live_reserve,
            fees,
        })
    }

    pub fn assert_market_invariants(&self) -> Result<()> {
        self.base_side.assert_share_backing()?;
        self.quote_side.assert_share_backing()?;
        self.base_side.fees.assert_backed()?;
        self.quote_side.fees.assert_backed()?;
        Ok(())
    }

    pub fn spot_value_in_opposite(&self, asset: MarketAsset, amount: u64) -> Result<u64> {
        require!(amount > 0, ErrorCode::AmountZero);
        let (from_reserve, to_reserve) = match asset {
            MarketAsset::Base => (
                self.base_side.reserves.live_reserve,
                self.quote_side.reserves.live_reserve,
            ),
            MarketAsset::Quote => (
                self.quote_side.reserves.live_reserve,
                self.base_side.reserves.live_reserve,
            ),
        };
        require!(
            from_reserve > 0 && to_reserve > 0,
            ErrorCode::InsufficientLiquidity
        );
        let value = (amount as u128)
            .checked_mul(to_reserve as u128)
            .and_then(|value| value.checked_div(from_reserve as u128))
            .ok_or(ErrorCode::MarketMathOverflow)?;
        u64::try_from(value).map_err(|_| ErrorCode::MarketMathOverflow.into())
    }
}

fn accrue_side(market: &mut Market, asset: MarketAsset, dt_ms: u64) -> Result<()> {
    let (index, rate_at_target) = match asset {
        MarketAsset::Base => (
            market.debt.base_borrow_index_nad,
            market.debt.base_rate_at_target_nad,
        ),
        MarketAsset::Quote => (
            market.debt.quote_borrow_index_nad,
            market.debt.quote_rate_at_target_nad,
        ),
    };
    let cash = match asset {
        MarketAsset::Base => market.base_side.reserves.cash_reserve,
        MarketAsset::Quote => market.quote_side.reserves.cash_reserve,
    } as u128;

    let borrowed = total_borrowed(market, asset, index)?;
    let util = utilization_bps(borrowed, cash)?;
    let error = utilization_error_nad(util, INTEREST_TARGET_UTILIZATION_BPS)?;
    let rate = instantaneous_rate_apr_nad(rate_at_target, error, INTEREST_CURVE_STEEPNESS_NAD)?;
    let next_index = accrued_index_nad(index, rate, dt_ms)?;
    let next_rate_at_target = adapt_rate_at_target_nad(
        rate_at_target,
        error,
        dt_ms,
        INTEREST_ADJUSTMENT_SPEED_PER_YEAR,
        INTEREST_MIN_RATE_AT_TARGET_NAD,
        INTEREST_MAX_RATE_AT_TARGET_NAD,
        INTEREST_MAX_ADAPTATION_STEP_NAD,
    )?;

    match asset {
        MarketAsset::Base => {
            market.debt.base_borrow_index_nad = next_index;
            market.debt.base_rate_at_target_nad = next_rate_at_target;
        }
        MarketAsset::Quote => {
            market.debt.quote_borrow_index_nad = next_index;
            market.debt.quote_rate_at_target_nad = next_rate_at_target;
        }
    }
    Ok(())
}

fn total_borrowed(market: &Market, asset: MarketAsset, index_nad: u128) -> Result<u128> {
    let (margin_fixed, hlp_shares) = match asset {
        MarketAsset::Base => (
            market.debt.fixed_base_shares,
            market.quote_hlp_vault.debt_shares,
        ),
        MarketAsset::Quote => (
            market.debt.fixed_quote_shares,
            market.base_hlp_vault.debt_shares,
        ),
    };
    let total_shares = margin_fixed
        .checked_add(hlp_shares)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Debt::shares_to_debt(total_shares, index_nad)
}

fn sync_borrow_recognition(
    market: &mut Market,
    margin_position: &mut MarginPosition,
    debt_asset: MarketAsset,
) -> Result<()> {
    let risk = market.current_risk()?;
    let recognition_slot = Clock::get()
        .map(|clock| clock.slot)
        .unwrap_or(market.last_update_slot);

    match debt_asset {
        MarketAsset::Base => {
            let old_recognized = margin_position.recognized_quote_collateral_for_base_debt;
            let target_recognized =
                market.debt_capped_recognized_collateral(margin_position, debt_asset, &risk)?;
            reconcile_recognition(
                &mut margin_position.recognized_quote_collateral_for_base_debt,
                &mut market.debt.recognized_quote_collateral_for_base_debt,
                old_recognized,
                target_recognized,
            )?;
        }
        MarketAsset::Quote => {
            let old_recognized = margin_position.recognized_base_collateral_for_quote_debt;
            let target_recognized =
                market.debt_capped_recognized_collateral(margin_position, debt_asset, &risk)?;
            reconcile_recognition(
                &mut margin_position.recognized_base_collateral_for_quote_debt,
                &mut market.debt.recognized_base_collateral_for_quote_debt,
                old_recognized,
                target_recognized,
            )?;
        }
    }

    market.debt.last_recognition_slot = recognition_slot;
    Ok(())
}

fn reconcile_recognition(
    position_recognized: &mut u64,
    market_recognized: &mut u64,
    old_recognized: u64,
    target_recognized: u64,
) -> Result<()> {
    match target_recognized.cmp(&old_recognized) {
        std::cmp::Ordering::Greater => {
            let delta = target_recognized
                .checked_sub(old_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *market_recognized = market_recognized
                .checked_add(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Less => {
            let delta = old_recognized
                .checked_sub(target_recognized)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            *market_recognized = market_recognized
                .checked_sub(delta)
                .ok_or(ErrorCode::MarketMathOverflow)?;
        }
        std::cmp::Ordering::Equal => {}
    }

    *position_recognized = target_recognized;
    Ok(())
}

fn require_borrow_headroom(debt_side: &MarketSide, borrow_amount: u64) -> Result<()> {
    require_gte!(
        debt_side.reserves.cash_reserve,
        borrow_amount,
        ErrorCode::InsufficientBorrowHeadroom
    );
    Ok(())
}

fn proportional_release(recognized: u64, shares_to_burn: u128, shares_before: u128) -> Result<u64> {
    require!(shares_before > 0, ErrorCode::InsufficientDebt);
    if shares_to_burn == shares_before {
        return Ok(recognized);
    }
    let release = (recognized as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflow)?;
    u64::try_from(release).map_err(|_| ErrorCode::MarketMathOverflow.into())
}

#[macro_export]
macro_rules! generate_market_seeds {
    ($market:expr) => {
        [
            MARKET_V2_SEED_PREFIX,
            $market.base_mint.as_ref(),
            $market.quote_mint.as_ref(),
            $market.params_hash.as_ref(),
            &[$market.bump],
        ]
    };
}

#[cfg(test)]
mod tests {
    include!("../../tests/state/market.rs");
}

#[cfg(test)]
mod reserve_tests {
    include!("../../tests/transitions/reserve.rs");
}

#[cfg(test)]
mod interest_tests {
    include!("../../tests/transitions/interest.rs");
}
