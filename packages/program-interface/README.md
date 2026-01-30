# @omnipair/program-interface

TypeScript interface for the [Omnipair](https://omnipair.fi) Solana program - an oracleless spot and margin money market protocol.

## Installation

```bash
npm install @omnipair/program-interface
# or
yarn add @omnipair/program-interface
```

## Usage

```typescript
import { Program } from "@coral-xyz/anchor";
import { IDL, Omnipair, PROGRAM_ID, derivePairAddress } from "@omnipair/program-interface";

// Create a typed program instance
const program = new Program<Omnipair>(IDL, PROGRAM_ID, provider);

// Fetch a pair account (fully typed)
const [pairAddress] = derivePairAddress(token0, token1);
const pair = await program.account.pair.fetch(pairAddress);

console.log("Reserve0:", pair.reserve0.toString());
console.log("Reserve1:", pair.reserve1.toString());
```

## Exports

### IDL
The Anchor IDL JSON for the Omnipair program.

### Types
All TypeScript types generated from the IDL:
- `Omnipair` - The program type
- Account types: `Pair`, `UserPosition`, `RateModel`, `FutarchyAuthority`
- Instruction argument types
- Event types

### Constants
- `PROGRAM_ID` - The Omnipair program ID
- `DEPLOYER_DEV` - Development deployer address
- `DEPLOYER_PROD` - Production deployer address
- `SEEDS` - PDA seed constants

### Utilities
- `derivePairAddress(token0, token1)` - Derive a Pair PDA
- `deriveUserPositionAddress(pair, user)` - Derive a UserPosition PDA

## Peer Dependencies

- `@coral-xyz/anchor` >= 0.30.0

## License

MIT
