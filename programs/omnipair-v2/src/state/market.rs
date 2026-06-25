use anchor_lang::prelude::*;

use super::{
    Debt, FutarchyAuthority, HlpVault, Insurance, MarketAsset, MarketConfig, MarketHealth,
    MarketSide, Risk,
};
use crate::constants::*;
use crate::errors::ErrorCode;

#[account]
#[derive(InitSpace)]
pub struct Market {
    pub version: u8,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
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
        operator: Pubkey,
        manager: Pubkey,
        base_side: MarketSide,
        quote_side: MarketSide,
        config: MarketConfig,
        base_hlp_base_ylp_vault: Pubkey,
        base_hlp_quote_ylp_vault: Pubkey,
        quote_hlp_base_ylp_vault: Pubkey,
        quote_hlp_quote_ylp_vault: Pubkey,
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
                vault.initialize(
                    MarketAsset::Base,
                    base_hlp_base_ylp_vault,
                    base_hlp_quote_ylp_vault,
                    current_slot,
                );
                vault
            },
            quote_hlp_vault: {
                let mut vault = HlpVault::default();
                vault.initialize(
                    MarketAsset::Quote,
                    quote_hlp_base_ylp_vault,
                    quote_hlp_quote_ylp_vault,
                    current_slot,
                );
                vault
            },
            risk: Risk {
                last_snapshot_slot: current_slot,
                ..Risk::default()
            },
            health: MarketHealth::default(),
            insurance: Insurance::default(),
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
    use super::*;

    fn market_with_roles(manager: Pubkey, operator: Pubkey) -> Market {
        Market {
            version: MARKET_VERSION,
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            operator,
            manager,
            base_side: MarketSide::default(),
            quote_side: MarketSide::default(),
            config: MarketConfig::default(),
            debt: Debt::default(),
            base_hlp_vault: HlpVault::default(),
            quote_hlp_vault: HlpVault::default(),
            risk: Risk::default(),
            health: MarketHealth::default(),
            insurance: Insurance::default(),
            params_hash: [0u8; 32],
            last_update_slot: 0,
            reduce_only: false,
            bump: 255,
        }
    }

    #[test]
    fn assert_manager_accepts_only_the_manager() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let market = market_with_roles(manager, operator);
        assert!(market.assert_manager(manager).is_ok());
        // The operator is NOT the manager for sensitive (manager-only) actions.
        assert!(market.assert_manager(operator).is_err());
        assert!(market.assert_manager(Pubkey::new_unique()).is_err());
    }

    #[test]
    fn assert_config_authority_accepts_only_manager() {
        let manager = Pubkey::new_unique();
        let operator = Pubkey::new_unique();
        let market = market_with_roles(manager, operator);
        assert!(market.assert_config_authority(manager).is_ok());
        assert!(market.assert_config_authority(operator).is_err());
        assert!(market
            .assert_config_authority(Pubkey::new_unique())
            .is_err());
    }
}
