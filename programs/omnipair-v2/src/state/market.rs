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
    use crate::{
        math::*,
        state::{
            BufferLedger, FeeLedger, HedgePosition, MarginPosition, MarketFeeClaimKind,
            StakePosition,
        },
        transitions::fee::{CarryForwardStakerFees, RecordFeeCredit},
        transitions::reserve::{AddLiquidity, RemoveLiquidity},
    };

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

    fn hedge_position() -> HedgePosition {
        HedgePosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            asset_mint: Pubkey::new_unique(),
            hedged_claim_token_amount: 0,
            fee_growth_checkpoint_nad: 0,
            accrued_fee_amount: 0,
            bump: 1,
        }
    }

    fn margin_position() -> MarginPosition {
        MarginPosition {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            base_collateral: 0,
            quote_collateral: 0,
            recognized_base_collateral_for_quote_debt: 0,
            recognized_quote_collateral_for_base_debt: 0,
            fixed_base_debt_shares: 0,
            fixed_quote_debt_shares: 0,
            bump: 1,
        }
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
    fn reserve_deposit_mints_claim_minus_buffer() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);

        let receipt = AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(receipt.claim_amount, 800_000);
        assert_eq!(receipt.buffer_amount, 200_000);
        assert_eq!(market_side.reserve_ledger.live_reserve, 1_000_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 1_000_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            800_000
        );
        assert_eq!(market_side.buffer_ledger.buffer_share_supply, 200_000);
        assert_eq!(market_side.buffer_ledger.required_buffer, 200_000);
        assert_eq!(market_side.claim_floor().unwrap(), 1_000_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn claim_redemption_is_fixed_one_to_one_principal() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
        AddLiquidity::new(1_000_000)
            .apply(&mut market_side)
            .unwrap();
        RecordFeeCredit::new(10_000, 0, 0, NAD)
            .apply(&mut market_side)
            .unwrap();

        RemoveLiquidity::new(100_000)
            .apply(&mut market_side)
            .unwrap();

        assert_eq!(market_side.reserve_ledger.live_reserve, 900_000);
        assert_eq!(market_side.reserve_ledger.cash_reserve, 900_000);
        assert_eq!(
            market_side.claim_token_ledger.protected_claim_token_supply,
            700_000
        );
        assert_eq!(market_side.buffer_ledger.required_buffer, 175_000);
        assert_eq!(market_side.fee_ledger.fee_vault_balance, 10_000);
        assert_eq!(market_side.fee_ledger.unallocated_fee_liability, 10_000);
        market_side.assert_claim_coverage().unwrap();
    }

    #[test]
    fn fee_ledger_allocates_only_to_matched_stake() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
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
    fn fee_ledger_routes_pressure_share_to_hedged_liability() {
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
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
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
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
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);

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
        let mut market_side = test_market_side(Pubkey::new_unique(), 2_000);
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

    #[test]
    fn market_fee_liabilities_settle_operator_and_protocol_buckets() {
        let mut fee_ledger = FeeLedger {
            fee_vault_balance: 700,
            operator_fee_liability: 400,
            protocol_fee_liability: 300,
            ..FeeLedger::default()
        };

        let operator_fee = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap();
        let protocol_fee = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Protocol)
            .unwrap();
        let err = fee_ledger
            .claim_market_fee_liability(MarketFeeClaimKind::Operator)
            .unwrap_err();

        assert_eq!(operator_fee, 400);
        assert_eq!(protocol_fee, 300);
        assert_eq!(fee_ledger.operator_fee_liability, 0);
        assert_eq!(fee_ledger.protocol_fee_liability, 0);
        assert_eq!(err, error!(ErrorCode::AmountZero));
    }

    #[test]
    fn stake_position_accrues_checkpointed_non_compounding_fees() {
        let mut position = stake_position();
        position.credit_buffer_share_amount(200_000).unwrap();
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

        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_add(500_000)
            .unwrap();
        position.increase(500_000).unwrap();

        assert_eq!(position.hedged_claim_token_amount, 500_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            500_000
        );
        assert_eq!(market_side.claim_token_ledger.staked_claim_token_supply, 0);
        assert_eq!(market_side.buffer_ledger.staked_buffer_share_amount, 0);

        position.decrease(125_000).unwrap();
        market_side.claim_token_ledger.hedged_claim_token_supply = market_side
            .claim_token_ledger
            .hedged_claim_token_supply
            .checked_sub(125_000)
            .unwrap();
        assert_eq!(position.hedged_claim_token_amount, 375_000);
        assert_eq!(
            market_side.claim_token_ledger.hedged_claim_token_supply,
            375_000
        );
    }

    #[test]
    fn hedge_position_accrues_checkpointed_routed_fees() {
        let mut position = hedge_position();
        position.increase(200_000).unwrap();
        position.fee_growth_checkpoint_nad = NAD as u128;

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
        assert_eq!(position.fee_growth_checkpoint_nad, 4 * NAD as u128);

        position.accrue_fees(4 * NAD as u128).unwrap();
        assert_eq!(position.accrued_fee_amount, 600_000);
    }

    #[test]
    fn market_health_uses_recognized_collateral_not_idle_inventory() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(1_000, NAD as u128).unwrap();

        market.refresh_market_health().unwrap();
        assert_eq!(market.health.base_debt_health_bps, 0);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealth)
        );

        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 1_500;
        market.refresh_market_health().unwrap();
        assert!(market.health.base_debt_health_bps >= 14_900);
        market.assert_market_health().unwrap();
    }

    #[test]
    fn market_health_rejects_raw_unit_decimal_pump() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let mut base_side = test_market_side(base_mint, 2_000);
        let mut quote_side = test_market_side(quote_mint, 2_000);
        base_side.asset_decimals = 6;
        quote_side.asset_decimals = 9;
        base_side.reserve_ledger.live_reserve = 1_000_000_000;
        quote_side.reserve_ledger.live_reserve = 1_000_000_000_000_000;
        let mut market = Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            base_side,
            quote_side,
            MarketConfig {
                recognized_collateral_cap_bps: 11_000,
                ..test_market().config
            },
            [8_u8; 32],
            42,
            253,
        )
        .unwrap();
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(900_000_000, NAD as u128).unwrap();
        market
            .recognition_ledger
            .debt_bearing_quote_collateral_for_base_debt = 1_000_000_000;
        market.refresh_market_health().unwrap();

        assert!(market.health.base_debt_health_bps < market.config.market_health_min_bps as u64);
        assert_eq!(
            market.assert_market_health().unwrap_err(),
            error!(ErrorCode::InsufficientMarketHealth)
        );
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

    #[test]
    fn daily_borrow_limit_uses_side_liquidity_ema() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000;
        market.refresh_risk_book().unwrap();

        market
            .enforce_daily_borrow_limit(MarketAsset::Base, 200_000)
            .unwrap();
        let err = market
            .enforce_daily_borrow_limit(MarketAsset::Base, 1)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::DailyLimitExceeded));
        assert_eq!(market.base_side.daily_limit_book.borrowed_bucket, 200_000);
    }

    #[test]
    fn daily_limit_rejects_zero_liquidity() {
        let mut market = test_market();

        let err = market
            .enforce_daily_borrow_limit(MarketAsset::Base, 1)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientLiquidity));
    }

    #[test]
    fn circuit_breaker_rejects_spot_ema_divergence() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 1_000_000;
        market.quote_side.reserve_ledger.live_reserve = 2_000_000;
        market.risk_book.base_price_ema_nad = NAD;
        market.risk_book.quote_price_ema_nad = NAD;

        let err = market.assert_spot_ema_divergence().unwrap_err();

        assert_eq!(err, error!(ErrorCode::MarketRiskCircuitBreaker));
    }

    #[test]
    fn circuit_breaker_rejects_k_ema_drawdown() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 900_000;
        market.quote_side.reserve_ledger.live_reserve = 900_000;
        market.risk_book.base_price_ema_nad = NAD;
        market.risk_book.quote_price_ema_nad = NAD;
        market.risk_book.k_ema = normalize_to_nad(1_000_000, market.base_side.asset_decimals)
            .unwrap()
            .checked_mul(normalize_to_nad(1_000_000, market.quote_side.asset_decimals).unwrap())
            .unwrap();

        market.assert_spot_ema_divergence().unwrap();
        let err = market.assert_risk_circuit_breakers().unwrap_err();

        assert_eq!(err, error!(ErrorCode::MarketRiskCircuitBreaker));
    }

    #[test]
    fn effective_debt_applies_gamma_only_to_hedged_overlay() {
        let mut market = test_market();
        market.base_side.reserve_ledger.live_reserve = 2_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 2_000_000_000;
        market.refresh_risk_book().unwrap();
        market.config.effective_debt_weight_min_bps = 5_000;
        market.config.effective_debt_gamma_nad = 2 * NAD;
        market.risk_book.liquidity_ema = 1_000 * NAD as u128;
        market.debt_book.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, NAD as u128).unwrap();
        market
            .quote_side
            .claim_token_ledger
            .hedged_claim_token_supply = 100_000_000;

        let raw_hedged_debt = market.hedged_base_debt_nad(&market.risk_book).unwrap();
        let effective_hedged_debt = effective_hedged_debt_nad(
            raw_hedged_debt,
            market.risk_book.liquidity_ema,
            market.config.effective_debt_weight_min_bps,
            market.config.effective_debt_gamma_nad,
        )
        .unwrap();
        let effective = market.effective_base_debt_nad().unwrap();

        assert!(raw_hedged_debt > 0);
        assert!(effective_hedged_debt < raw_hedged_debt);
        assert_eq!(effective, 100 * NAD as u128 + effective_hedged_debt);
    }

    #[test]
    fn recognized_collateral_is_capped_by_debt_value() {
        let mut market = test_market();
        market.config.recognized_collateral_cap_bps = 15_000;
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.refresh_risk_book().unwrap();
        let mut position = margin_position();
        position.quote_collateral = 1_000_000_000;
        position.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, market.debt_book.base_borrow_index_nad).unwrap();

        let recognized = market
            .debt_capped_recognized_collateral(&position, MarketAsset::Base, &market.risk_book)
            .unwrap();
        let recognized_value = market
            .collateral_value_nad(MarketAsset::Quote, recognized, &market.risk_book)
            .unwrap();
        let debt_value_cap = normalize_to_nad(100_000_000, market.base_side.asset_decimals)
            .unwrap()
            .checked_mul(15_000)
            .and_then(|value| value.checked_div(BPS_DENOMINATOR as u128))
            .unwrap();

        assert!(recognized > 100_000_000);
        assert!(recognized < position.quote_collateral);
        assert!(recognized_value <= debt_value_cap);
    }

    #[test]
    fn recognition_cap_rejects_idle_collateral_pump() {
        let mut market = test_market();
        market.config.recognized_collateral_cap_bps = 15_000;
        market.base_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.quote_side.reserve_ledger.live_reserve = 1_000_000_000;
        market.refresh_risk_book().unwrap();
        let mut position = margin_position();
        position.quote_collateral = 1_000_000_000;
        position.recognized_quote_collateral_for_base_debt = 1_000_000_000;
        position.fixed_base_debt_shares =
            DebtBook::debt_to_shares(100_000_000, market.debt_book.base_borrow_index_nad).unwrap();

        let err = market
            .assert_recognition_cap(&position, MarketAsset::Base)
            .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InsufficientRecognizedCollateral));
    }

    #[test]
    fn stale_recognition_cannot_exceed_margin_collateral() {
        let mut position = margin_position();
        position.base_collateral = 100;
        position.recognized_base_collateral_for_quote_debt = 80;
        assert_eq!(position.idle_base_collateral().unwrap(), 20);

        position.recognized_base_collateral_for_quote_debt = 101;
        assert_eq!(
            position.idle_base_collateral().unwrap_err(),
            error!(ErrorCode::InsufficientRecognizedCollateral)
        );
    }
}
