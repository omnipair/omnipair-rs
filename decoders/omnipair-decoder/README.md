# omnipair-decoder

Carbon decoder for [Omnipair](https://omnipair.fi) - a Solana oracleless spot and margin money market protocol.

## Installation

```toml
[dependencies]
omnipair-decoder = "0.1"
```

## Usage

```rust
use omnipair_decoder::{OmnipairDecoder, accounts::OmnipairAccount, instructions::OmnipairInstruction};
use carbon_core::account::AccountDecoder;
use carbon_core::instruction::InstructionDecoder;

let decoder = OmnipairDecoder;

// Decode an account
if let Some(decoded) = decoder.decode_account(&account) {
    match decoded.data {
        OmnipairAccount::Pair(pair) => {
            println!("Pair: {:?}", pair);
        }
        OmnipairAccount::UserPosition(pos) => {
            println!("User Position: {:?}", pos);
        }
        // ... other account types
        _ => {}
    }
}

// Decode an instruction
if let Some(decoded) = decoder.decode_instruction(&instruction) {
    match decoded.data {
        OmnipairInstruction::Swap(swap) => {
            println!("Swap: {:?}", swap);
        }
        // ... other instruction types
        _ => {}
    }
}
```

## Features

- Decode all Omnipair account types (Pair, UserPosition, RateModel, FutarchyAuthority)
- Decode all Omnipair instructions and events
- Full type definitions for all program types
- Compatible with [Carbon](https://github.com/sevenlabs-hq/carbon) indexing framework

## License

MIT
