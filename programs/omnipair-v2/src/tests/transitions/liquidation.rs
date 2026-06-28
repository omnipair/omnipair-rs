use super::*;
use crate::{
    constants::{BPS_DENOMINATOR, MARKET_VERSION, NAD},
    state::{
        Debt, HlpVault, Insurance, MarketConfig, MarketHealth, MarketSide, PendingAuthorityChange,
        PendingConfigChange, Reserves, Risk,
    },
};

fn valid_config() -> MarketConfig {
    MarketConfig {
        swap_fee_bps: 30,
        manager_fee_bps: 0,
        protocol_fee_bps: 0,
        target_hlp_leverage_bps: BPS_DENOMINATOR * 2,
        settlement_divergence_bps: 500,
        emergency_exit_haircut_bps: 250,
        ema_half_life_ms: 60_000,
        directional_ema_half_life_ms: 60_000,
        k_ema_half_life_ms: 60_000,
        max_daily_borrow_bps: 2_000,
        max_daily_withdraw_bps: 2_000,
        spot_ema_divergence_bps: 1_000,
        k_ema_drawdown_bps: 1_000,
        recognized_collateral_cap_bps: 15_000,
        market_health_min_bps: 11_000,
        liquidation_auction_duration_slots: 1_200,
        liquidation_auction_start_incentive_bps: 0,
        hedged_lp_enabled: true,
        start_time: 0,
    }
}

fn liquidatable_quote_debt_position() -> (Market, MarginPosition) {
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();
    let mut base_side = MarketSide {
        asset_mint: base_mint,
        asset_decimals: 0,
        ..MarketSide::default()
    };
    base_side.reserves = Reserves {
        live_reserve: 1_000_000_000,
        cash_reserve: 1_000_000_000,
        reserved_liability: 0,
    };
    let mut quote_side = MarketSide {
        asset_mint: quote_mint,
        asset_decimals: 0,
        ..MarketSide::default()
    };
    quote_side.reserves = Reserves {
        live_reserve: 1_000_000_000,
        cash_reserve: 1_000_000_000,
        reserved_liability: 0,
    };

    let debt = Debt {
        fixed_quote_shares: 100,
        quote_borrow_index_nad: NAD as u128,
        base_borrow_index_nad: NAD as u128,
        fixed_quote_principal: 100,
        recognized_base_collateral_for_quote_debt: 109,
        ..Debt::default()
    };
    let market = Market {
        version: MARKET_VERSION,
        base_mint,
        quote_mint,
        ylp_mint: Pubkey::new_unique(),
        operator: Pubkey::new_unique(),
        manager: Pubkey::new_unique(),
        base_side,
        quote_side,
        config: valid_config(),
        debt,
        base_hlp_vault: HlpVault::default(),
        quote_hlp_vault: HlpVault::default(),
        risk: Risk::default(),
        health: MarketHealth::default(),
        insurance: Insurance::default(),
        pending_config: PendingConfigChange::default(),
        pending_operator: PendingAuthorityChange::default(),
        pending_manager: PendingAuthorityChange::default(),
        params_hash: [9; 32],
        last_update_slot: 0,
        reduce_only: false,
        bump: 255,
    };
    let margin_position = MarginPosition {
        owner: Pubkey::new_unique(),
        market: Pubkey::new_unique(),
        base_collateral: 109,
        quote_collateral: 0,
        recognized_base_collateral_for_quote_debt: 109,
        recognized_quote_collateral_for_base_debt: 0,
        fixed_base_shares: 0,
        fixed_quote_shares: 100,
        risk_epoch: 0,
        bump: 255,
    };
    (market, margin_position)
}

#[test]
fn euler_style_incentive_grows_with_health_shortfall() {
    assert_eq!(liquidation_incentive_bps(10_999, 11_000), 100);
    assert_eq!(liquidation_incentive_bps(10_750, 11_000), 250);
    assert_eq!(liquidation_incentive_bps(9_000, 11_000), 500);
}

#[test]
fn insurance_funding_preserves_room_to_restore_health() {
    let config = valid_config();

    assert_eq!(liquidation_insurance_funding_bps(100, &config).unwrap(), 200);
    assert_eq!(liquidation_insurance_funding_bps(500, &config).unwrap(), 200);

    let mut tight_config = valid_config();
    tight_config.market_health_min_bps = 10_250;
    assert_eq!(
        liquidation_insurance_funding_bps(200, &tight_config).unwrap(),
        49
    );
}

#[test]
fn max_repay_caps_liquidation_to_restore_target_health() {
    let (market, margin_position) = liquidatable_quote_debt_position();
    let incentive_bps = liquidation_incentive_bps(10_900, 11_000);
    let insurance_bps = liquidation_insurance_funding_bps(incentive_bps, &market.config).unwrap();
    let cap = max_repay_to_restore_health(
        &market,
        &margin_position,
        MarketAsset::Quote,
        incentive_bps + insurance_bps,
    )
    .unwrap();

    assert!((15..=16).contains(&cap));
}

#[test]
fn auction_pricing_uses_reference_price_for_collateral_seizure() {
    let (market, _) = liquidatable_quote_debt_position();
    let pricing = LiquidationPricing::ReferencePrice {
        debt_per_collateral_price_nad: NAD as u64,
    };

    let seized = collateral_amount_for_debt_value_with_pricing(
        &market,
        MarketAsset::Quote,
        100,
        300,
        pricing,
    )
    .unwrap();
    let bidder_collateral = collateral_amount_for_debt_value_with_pricing(
        &market,
        MarketAsset::Quote,
        100,
        100,
        pricing,
    )
    .unwrap();

    assert_eq!(seized, 103);
    assert_eq!(bidder_collateral, 101);
}

#[test]
fn auction_restore_cap_uses_reference_price() {
    let (market, margin_position) = liquidatable_quote_debt_position();
    let pricing = LiquidationPricing::ReferencePrice {
        debt_per_collateral_price_nad: NAD as u64,
    };
    let cap = max_repay_to_restore_health_with_pricing(
        &market,
        &margin_position,
        MarketAsset::Quote,
        300,
        pricing,
    )
    .unwrap();

    assert!((15..=16).contains(&cap));
}

#[test]
fn liquidation_rejects_repay_above_restore_cap() {
    let (mut market, mut margin_position) = liquidatable_quote_debt_position();
    let incentive_bps = liquidation_incentive_bps(10_900, 11_000);
    let insurance_bps = liquidation_insurance_funding_bps(incentive_bps, &market.config).unwrap();
    let cap = max_repay_to_restore_health(
        &market,
        &margin_position,
        MarketAsset::Quote,
        incentive_bps + insurance_bps,
    )
    .unwrap();

    let terms = liquidation_terms(&market, &margin_position, MarketAsset::Quote).unwrap();
    let err = Liquidation::new(MarketAsset::Quote, cap + 1, 0, 0, 0, terms)
        .apply(&mut market, &mut margin_position)
        .unwrap_err();

    assert_eq!(
        err,
        anchor_lang::prelude::error!(ErrorCode::LiquidationRepayTooLarge)
    );
}
