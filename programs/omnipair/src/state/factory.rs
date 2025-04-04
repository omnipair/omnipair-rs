use anchor_lang::prelude::*;

#[account]
pub struct Factory {
    pub owner: Pubkey,
    pub pair_count: u64,
    pub all_pairs: Vec<Pubkey>, // Registry of Pair addresses
}

impl Factory {
    // Maximum number of pairs (for fixed storage size)
    pub const MAX_PAIRS: usize = 1000;
    // Calculate the size required for the Factory account.
    pub const SIZE: usize = 32 + 8 + 4 + (Self::MAX_PAIRS * 32);
}
