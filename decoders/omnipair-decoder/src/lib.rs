//! # omnipair-decoder
//!
//! Carbon decoder for Omnipair - Solana oracleless spot and margin money market protocol.
//!
//! This crate provides account and instruction decoders for the Omnipair program,
//! compatible with the [Carbon](https://github.com/sevenlabs-hq/carbon) indexing framework.
//!
//! ## Example
//!
//! ```ignore
//! use omnipair_decoder::{OmnipairDecoder, accounts::OmnipairAccount};
//! use carbon_core::account::AccountDecoder;
//!
//! let decoder = OmnipairDecoder;
//! if let Some(decoded) = decoder.decode_account(&account) {
//!     // Handle decoded account
//! }
//! ```

pub mod accounts;
pub mod instructions;
pub mod types;

pub use accounts::OmnipairAccount;
pub use instructions::OmnipairInstruction;

/// The main decoder struct for Omnipair program.
///
/// Implements `AccountDecoder` and `InstructionDecoder` traits from carbon-core.
#[derive(Debug, Clone, Copy, Default)]
pub struct OmnipairDecoder;

/// Omnipair program ID
pub const PROGRAM_ID: solana_pubkey::Pubkey =
    solana_pubkey::pubkey!("omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb");