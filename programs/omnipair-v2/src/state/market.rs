use anchor_lang::prelude::*;

use super::{
    DebtBook, InsuranceReserve, MarketAsset, MarketConfig, MarketHealth, MarketSide,
    RecognitionLedger, RiskBook,
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
    pub debt_book: DebtBook,
    pub risk_book: RiskBook,
    pub health: MarketHealth,
    pub recognition_ledger: RecognitionLedger,
    pub insurance_reserve: InsuranceReserve,
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
            debt_book: DebtBook {
                base_borrow_index_nad: NAD as u128,
                quote_borrow_index_nad: NAD as u128,
                ..DebtBook::default()
            },
            risk_book: RiskBook {
                last_snapshot_slot: current_slot,
                ..RiskBook::default()
            },
            health: MarketHealth::default(),
            recognition_ledger: RecognitionLedger {
                last_recognition_slot: current_slot,
                ..RecognitionLedger::default()
            },
            insurance_reserve: InsuranceReserve::default(),
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

    pub fn assert_started(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= self.config.start_time, ErrorCode::MarketNotStarted);
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

    pub fn assert_market_invariants(&self) -> Result<()> {
        self.base_side.assert_claim_coverage()?;
        self.quote_side.assert_claim_coverage()?;
        self.base_side.fee_ledger.assert_backed()?;
        self.quote_side.fee_ledger.assert_backed()?;
        Ok(())
    }

    pub fn apply_buffer_ratio_update(&mut self, buffer_ratio_bps: u16) -> Result<()> {
        self.assert_buffer_ratio_change_unlocked(buffer_ratio_bps)?;
        let required_buffer0 = self
            .base_side
            .assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        let required_buffer1 = self
            .quote_side
            .assert_buffer_floor_for_ratio(buffer_ratio_bps)?;
        self.base_side
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer0);
        self.quote_side
            .apply_buffer_ratio(buffer_ratio_bps, required_buffer1);
        Ok(())
    }

    fn assert_buffer_ratio_change_unlocked(&self, buffer_ratio_bps: u16) -> Result<()> {
        if buffer_ratio_bps == self.base_side.buffer_ledger.buffer_ratio_bps
            && buffer_ratio_bps == self.quote_side.buffer_ledger.buffer_ratio_bps
        {
            return Ok(());
        }
        require!(
            self.base_side.claim_token_ledger.staked_claim_token_supply == 0
                && self.quote_side.claim_token_ledger.staked_claim_token_supply == 0
                && self.base_side.buffer_ledger.staked_buffer_share_amount == 0
                && self.quote_side.buffer_ledger.staked_buffer_share_amount == 0
                && self.base_side.fee_ledger.fee_liability == 0
                && self.quote_side.fee_ledger.fee_liability == 0,
            ErrorCode::InvalidMarketConfig
        );
        Ok(())
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
    use crate::{state::BufferLedger, transitions::reserve::AddLiquidity};

    fn test_market_side(asset_mint: Pubkey, buffer_ratio_bps: u16) -> MarketSide {
        MarketSide {
            asset_mint,
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            buffer_ledger: BufferLedger {
                buffer_ratio_bps,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            test_market_side(base_mint, 2_000),
            test_market_side(quote_mint, 2_000),
            MarketConfig {
                swap_fee_bps: 30,
                operator_fee_bps: 1_000,
                protocol_fee_bps: 0,
                buffer_ratio_bps: 2_000,
                fee_routing_k_nad: NAD,
                ema_half_life_ms: 60_000,
                directional_ema_half_life_ms: 60_000,
                k_ema_half_life_ms: 60_000,
                max_daily_borrow_bps: 2_000,
                max_daily_withdraw_bps: 2_000,
                spot_ema_divergence_bps: 1_000,
                k_ema_drawdown_bps: 1_000,
                recognized_collateral_cap_bps: 15_000,
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

    #[test]
    fn market_initialize_preserves_creator_chosen_base_quote_order() {
        let base_mint = Pubkey::new_from_array([2_u8; 32]);
        let quote_mint = Pubkey::new_from_array([1_u8; 32]);
        let market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            test_market_side(base_mint, 2_000),
            test_market_side(quote_mint, 2_000),
            test_market().config,
            [7_u8; 32],
            42,
            254,
        )
        .unwrap();

        assert_eq!(market.base_mint, base_mint);
        assert_eq!(market.quote_mint, quote_mint);
        assert_eq!(market.base_side.asset_mint, base_mint);
        assert_eq!(market.quote_side.asset_mint, quote_mint);
    }

    #[test]
    fn market_initialize_rejects_default_authorities() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let base_side = test_market_side(base_mint, 2_000);
        let quote_side = test_market_side(quote_mint, 2_000);
        let config = test_market().config;

        let default_operator = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::default(),
            Pubkey::new_unique(),
            base_side,
            quote_side,
            config,
            [7_u8; 32],
            42,
            254,
        )
        .err()
        .unwrap();
        let default_manager = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::default(),
            base_side,
            quote_side,
            config,
            [7_u8; 32],
            42,
            254,
        )
        .err()
        .unwrap();

        assert_eq!(default_operator, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(default_manager, error!(ErrorCode::InvalidMarketConfig));
    }

    #[test]
    fn buffer_ratio_update_recomputes_required_floor() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(2_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market.base_side.buffer_ledger.buffer_share_supply += 100_000;
        market.base_side.reserve_ledger.live_reserve += 100_000;
        market.quote_side.buffer_ledger.buffer_share_supply += 200_000;
        market.quote_side.reserve_ledger.live_reserve += 200_000;

        market.apply_buffer_ratio_update(2_500).unwrap();

        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_500);
        assert_eq!(market.base_side.buffer_ledger.required_buffer, 266_667);
        assert_eq!(market.quote_side.buffer_ledger.required_buffer, 533_334);
    }

    #[test]
    fn buffer_ratio_update_rejects_uncovered_floor() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();

        let err = market.apply_buffer_ratio_update(2_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientBufferShares));
        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_000);
        assert_eq!(market.base_side.buffer_ledger.required_buffer, 200_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_active_stake() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market
            .base_side
            .claim_token_ledger
            .staked_claim_token_supply = 800_000;
        market.base_side.buffer_ledger.staked_buffer_share_amount = 200_000;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(market.base_side.buffer_ledger.buffer_ratio_bps, 2_000);
    }

    #[test]
    fn buffer_ratio_update_rejects_staker_fee_liability() {
        let mut market = test_market();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.base_side)
            .unwrap();
        AddLiquidity::new(1_000_000)
            .apply(&mut market.quote_side)
            .unwrap();
        market.quote_side.fee_ledger.fee_liability = 1;

        let err = market.apply_buffer_ratio_update(1_500).unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidMarketConfig));
        assert_eq!(market.quote_side.buffer_ledger.buffer_ratio_bps, 2_000);
    }
}
