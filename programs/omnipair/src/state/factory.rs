use anchor_lang::prelude::*;

#[account]
pub struct Factory {
    pub owner: Pubkey,
    pub pair_count: u64,
    pub pair_registry: Pubkey, // Reference to the first PairRegistry account
}

impl Factory {
    // Calculate the size required for the Factory account.
    // 8 bytes for discriminator
    // 32 bytes for owner
    // 8 bytes for pair_count
    // 32 bytes for pair_registry
    pub const SIZE: usize = 8 + 32 + 8 + 32;

    pub fn get_factory_address(owner: Pubkey, program_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"factory", owner.as_ref()],
            &program_id
        )
    }
}

#[account]
pub struct PairRegistry {
    pub factory: Pubkey,
    pub next_registry: Option<Pubkey>, // Link to the next registry if needed
    pub registry_index: u32, // Index of this registry (0 for the first one)
    pub pairs: Vec<Pubkey>, // Registry of Pair addresses
}

impl PairRegistry {
    // Maximum number of pairs per registry (to stay under 10KB)
    pub const MAX_PAIRS_PER_REGISTRY: usize = 250;
    // Calculate the size required for the PairRegistry account.
    // 8 bytes for discriminator
    // 32 bytes for factory
    // 32 bytes for next_registry (Option<Pubkey>)
    // 4 bytes for registry_index
    // 4 bytes for vector length
    // MAX_PAIRS_PER_REGISTRY * 32 bytes for vector data
    pub const SIZE: usize = 8 + 32 + 32 + 4 + 4 + (Self::MAX_PAIRS_PER_REGISTRY * 32);
    
    // Helper function to get the PDA for a specific registry index
    pub fn get_registry_address(factory: Pubkey, index: u32, program_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"pair_registry", factory.as_ref(), &index.to_le_bytes()],
            &program_id
        )
    }
}
