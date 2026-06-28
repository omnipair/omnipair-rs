use anchor_lang::prelude::*;

use super::{
    Debt, FutarchyAuthority, HlpVault, Insurance, MarketAsset, MarketConfig, MarketHealth,
    MarketSide, Risk,
};
use crate::constants::*;
use crate::errors::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketTimelockAction {
    Scheduled { execute_after_slot: u64 },
    Ready,
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
        crate::transitions::interest::AccrueInterest::new(current_slot).apply(self)
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
    include!("../tests/state/market.rs");
}
