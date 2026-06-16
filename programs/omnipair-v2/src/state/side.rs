use anchor_lang::prelude::*;

use super::{BufferBook, ClaimLedger, DailyLimitBook, FeeLedger, ReserveLedger};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct MarketSide {
    pub asset_mint: Pubkey,
    pub asset_decimals: u8,
    pub claim_mint: Pubkey,
    pub hedge_mint: Pubkey,
    pub hedge_vault: Pubkey,
    pub reserve_vault: Pubkey,
    pub collateral_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub stake_vault: Pubkey,
    pub reserve_ledger: ReserveLedger,
    pub claim_ledger: ClaimLedger,
    pub buffer_book: BufferBook,
    pub fee_ledger: FeeLedger,
    pub daily_limit_book: DailyLimitBook,
}
