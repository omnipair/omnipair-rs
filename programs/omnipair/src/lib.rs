use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

// Re-export modules to simplify the program interface.
pub use state::*;
pub use utils::*;
pub use instructions::*;

declare_id!("GZqkUaCeaf96tm2Jw1QaY88fduMHnP7bhLTwjqDk6LM6");

#[program]
pub mod omnipair {
    use super::*;

    // Factory instructions
    pub fn initialize_factory(ctx: Context<InitializeFactory>, owner: Pubkey) -> Result<()> {
        instructions::initialize_factory(ctx, owner)
    }

    pub fn create_pair(ctx: Context<CreatePair>, rate_model: Pubkey) -> Result<()> {
        instructions::factory_create_pair::create_pair(ctx, rate_model)
    }
}
