use super::*;

fn open_auction() -> LiquidationAuction {
    let mut auction = LiquidationAuction {
        bump: 255,
        ..LiquidationAuction::default()
    };
    auction
        .open(OpenLiquidationAuctionParams {
            market: Pubkey::new_unique(),
            margin_position: Pubkey::new_unique(),
            borrower: Pubkey::new_unique(),
            debt_asset: MarketAsset::Quote,
            debt_mint: Pubkey::new_unique(),
            collateral_mint: Pubkey::new_unique(),
            position_risk_epoch: 7,
            current_slot: 100,
            duration_slots: 40,
            start_health_bps: 10_900,
            start_incentive_bps: 0,
            max_incentive_bps: 500,
            max_repay_amount: 1_000,
            reference_price_nad: crate::constants::NAD,
            bump: 255,
        })
        .unwrap();
    auction
}

#[test]
fn dutch_incentive_decays_from_start_to_live_max() {
    let auction = open_auction();

    assert_eq!(auction.current_incentive_bps(100, 500).unwrap(), 0);
    assert_eq!(auction.current_incentive_bps(120, 500).unwrap(), 250);
    assert_eq!(auction.current_incentive_bps(140, 500).unwrap(), 500);
    assert_eq!(auction.current_incentive_bps(200, 500).unwrap(), 500);
}

#[test]
fn dutch_incentive_respects_live_max_after_health_changes() {
    let auction = open_auction();

    assert_eq!(auction.current_incentive_bps(140, 250).unwrap(), 250);
}

#[test]
fn stale_position_epoch_cannot_settle() {
    let auction = open_auction();
    let margin_position = MarginPosition {
        owner: auction.borrower,
        market: auction.market,
        base_collateral: 0,
        quote_collateral: 0,
        recognized_base_collateral_for_quote_debt: 0,
        recognized_quote_collateral_for_base_debt: 0,
        fixed_base_shares: 0,
        fixed_quote_shares: 0,
        risk_epoch: 8,
        bump: 255,
    };

    let err = auction
        .assert_matches(
            auction.market,
            auction.margin_position,
            &margin_position,
            MarketAsset::Quote,
            auction.debt_mint,
            auction.collateral_mint,
        )
        .unwrap_err();

    assert_eq!(
        err,
        anchor_lang::prelude::error!(ErrorCode::StaleLiquidationAuction)
    );
}
