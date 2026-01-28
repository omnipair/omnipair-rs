use trident_fuzz::fuzzing::{pubkey, Pubkey};

// DEPLOYER
pub const DEPLOYER_ADDRESS: Pubkey = pubkey!("C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds");

// PROGRAMS
pub const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const MPL_TOKEN_METADATA_ID: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const FLASHLOAN_CALLBACK_RECEIVER_PROGRAM: Pubkey =
    pubkey!("GmtswKBDrFZ9DfUfP7jbPFvbtuG7AJcX73SvoKWGxJbu");

// MINTS
pub const WSOL_MINT_ADDRESS: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

// SEEDS
pub const FUTARCHY_AUTHORITY_SEED_PREFIX: &[u8] = b"futarchy_authority";
pub const METADATA_SEED_PREFIX: &[u8] = b"metadata";
pub const PAIR_SEED_PREFIX: &[u8] = b"gamm_pair";
pub const POSITION_SEED_PREFIX: &[u8] = b"gamm_position";

// EVENT AUTHORITY
pub const EVENT_AUTHORITY_ADDRESS: Pubkey = pubkey!("fY27dnRLq4XVAKNRAY7nATiicimnY6mwLHq3V65uoP2");
