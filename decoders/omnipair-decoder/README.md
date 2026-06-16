# omnipair-decoder

Carbon decoder for [Omnipair](https://omnipair.fi) - a Solana oracleless spot and margin money market protocol. The root decoder covers the legacy V1 pair program; the `v2` module covers the V2 market architecture program.

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

For V2:

```rust
use omnipair_decoder::v2::{
    OmnipairV2Decoder,
    accounts::OmnipairV2Account,
    instructions::OmnipairV2Instruction,
};
use carbon_core::account::AccountDecoder;
use carbon_core::instruction::InstructionDecoder;

let decoder = OmnipairV2Decoder;

if let Some(decoded) = decoder.decode_account(&account) {
    match decoded.data {
        OmnipairV2Account::Market(market) => {
            println!("Market: {:?}", market);
        }
        OmnipairV2Account::StakePosition(position) => {
            println!("Stake position: {:?}", position);
        }
        _ => {}
    }
}

if let Some(decoded) = decoder.decode_instruction(&instruction) {
    match decoded.data {
        OmnipairV2Instruction::Swap(swap) => {
            println!("V2 swap: {:?}", swap);
        }
        OmnipairV2Instruction::MarketCreated(event) => {
            println!("V2 market created: {:?}", event);
        }
        _ => {}
    }
}
```

## Features

- Decode legacy V1 account types (Pair, UserPosition, RateModel, FutarchyAuthority)
- Decode V2 account types (Market, MarginPosition, StakePosition, HedgePosition)
- Decode all V1 and V2 instructions and events
- Full type definitions for all program types
- Compatible with [Carbon](https://github.com/sevenlabs-hq/carbon) indexing framework

## License

MIT
