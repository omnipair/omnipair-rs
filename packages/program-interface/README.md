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
import { IDL, PROGRAM_ID, derivePairAddress } from "@omnipair/program-interface";
import type { Omnipair } from "@omnipair/program-interface";

// Create a typed program instance
const program = new Program<Omnipair>(IDL, PROGRAM_ID, provider);

// Fetch a pair account (fully typed)
const [pairAddress] = derivePairAddress(token0, token1, paramsHash);
const pair = await program.account.pair.fetch(pairAddress);

console.log("Reserve0:", pair.reserve0.toString());
console.log("Reserve1:", pair.reserve1.toString());
```

### Derive Pair PDA (full example)

`derivePairAddress` requires the same `paramsHash` used by the on-chain initialize instruction.

```typescript
import { PublicKey } from "@solana/web3.js";
import { createHash } from "node:crypto";
import { derivePairAddress } from "@omnipair/program-interface";

type InitParams = {
  version: number;
  swapFeeBps: number;
  halfLife: bigint;
  fixedCfBps?: number;
  targetUtilStartBps?: bigint;
  targetUtilEndBps?: bigint;
  rateHalfLifeMs?: bigint;
  minRateBps?: bigint;
  maxRateBps?: bigint;
};

function u16le(value: number): Buffer {
  const b = Buffer.alloc(2);
  b.writeUInt16LE(value, 0);
  return b;
}

function u64le(value: bigint): Buffer {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(value, 0);
  return b;
}

function computeParamsHash(params: InitParams): Uint8Array {
  const payload = Buffer.concat([
    Buffer.from([params.version]), // u8
    u16le(params.swapFeeBps), // u16
    u64le(params.halfLife), // u64
    u16le(params.fixedCfBps ?? 0), // Option<u16> encoded as unwrap_or(0)
    u64le(params.targetUtilStartBps ?? 0n), // Option<u64> unwrap_or(0)
    u64le(params.targetUtilEndBps ?? 0n),
    u64le(params.rateHalfLifeMs ?? 0n),
    u64le(params.minRateBps ?? 0n),
    u64le(params.maxRateBps ?? 0n),
  ]);

  return createHash("sha256").update(payload).digest();
}

const token0 = new PublicKey("So11111111111111111111111111111111111111112");
const token1 = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

const paramsHash = computeParamsHash({
  version: 1,
  swapFeeBps: 30,
  halfLife: 3_600_000n,
  fixedCfBps: undefined,
  targetUtilStartBps: 3_000n,
  targetUtilEndBps: 5_000n,
  rateHalfLifeMs: 259_200_000n,
  minRateBps: 100n,
  maxRateBps: 0n,
});

const [pairPda, pairBump] = derivePairAddress(token0, token1, paramsHash);
console.log("pair:", pairPda.toBase58(), "bump:", pairBump);
```

### JavaScript (runtime-only imports)

```javascript
import { IDL, PROGRAM_ID, derivePairAddress } from "@omnipair/program-interface";
```

`Omnipair` is a TypeScript type export, not a runtime JavaScript value. In TypeScript, import it with `import type { Omnipair } ...`.

## ESM Compatibility

This package ships strict ESM-compatible output (Node/tsx/bundlers). Relative module specifiers include `.js` extensions in emitted files, so usage works in strict ESM runtimes.

## Exports

### IDL
The Anchor IDL JSON for the Omnipair program.

### Types
All TypeScript types generated from the IDL:
- `Omnipair` - The program type (type-only export)
- Account types: `Pair`, `UserPosition`, `RateModel`, `FutarchyAuthority`
- Instruction argument types
- Event types

### Constants
- `PROGRAM_ID` - The Omnipair program ID
- `SEEDS` - PDA seed constants

### Utilities
- `derivePairAddress(token0, token1, paramsHash)` - Derive a Pair PDA
- `deriveUserPositionAddress(pair, user)` - Derive a UserPosition PDA
- `deriveFutarchyAuthorityAddress()` - Derive FutarchyAuthority PDA
- `deriveReserveVaultAddress(pair, reserveMint)` - Derive a reserve vault PDA
- `deriveCollateralVaultAddress(pair, collateralMint)` - Derive a collateral vault PDA

## Peer Dependencies

- `@coral-xyz/anchor` >= 0.30.0

## License

MIT
