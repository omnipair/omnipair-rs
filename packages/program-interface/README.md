# @omnipair/program-interface

TypeScript interface for the [Omnipair](https://omnipair.fi) Solana programs: the legacy V1 pair program and the V2 market architecture program.

## Step 1: Install

```bash
npm install @omnipair/program-interface
# or
yarn add @omnipair/program-interface
```

## Step 2: Create Anchor provider and program

```typescript
import * as anchor from "@coral-xyz/anchor";
import type { Omnipair } from "@omnipair/program-interface";
import { IDL, PROGRAM_ID } from "@omnipair/program-interface";

const connection = new anchor.web3.Connection(
  process.env.ANCHOR_PROVIDER_URL ?? "https://api.mainnet-beta.solana.com",
  "confirmed"
);
const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
const provider = new anchor.AnchorProvider(connection, wallet, {
  commitment: "confirmed",
});
const v1Program = new anchor.Program<Omnipair>(IDL, PROGRAM_ID, provider);
```

For V2:

```typescript
import * as anchor from "@coral-xyz/anchor";
import type { OmnipairV2 } from "@omnipair/program-interface";
import { IDL_V2, OMNIPAIR_V2_PROGRAM_ID } from "@omnipair/program-interface";

const v2Program = new anchor.Program<OmnipairV2>(IDL_V2, OMNIPAIR_V2_PROGRAM_ID, provider);
```

## Step 3: Compute legacy V1 `paramsHash`

`derivePairAddress` is for the legacy V1 pair program and requires the same `paramsHash` used by the V1 on-chain initialize instruction.

```typescript
import { createHash } from "node:crypto";

export type InitParams = {
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
```

## Step 4: Derive legacy V1 pair PDA and fetch account

```typescript
import { PublicKey } from "@solana/web3.js";
import { derivePairAddress } from "@omnipair/program-interface";

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

const pair = await v1Program.account.pair.fetch(pairPda);
console.log("Reserve0:", pair.reserve0.toString());
console.log("Reserve1:", pair.reserve1.toString());
```

## V2 market PDA example

V2 uses standalone market accounts. Pass the same `paramsHash` that was supplied to the V2 `initialize` instruction.

```typescript
import { PublicKey } from "@solana/web3.js";
import { deriveMarketAddress } from "@omnipair/program-interface";

const baseMint = new PublicKey("So11111111111111111111111111111111111111112");
const quoteMint = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const paramsHash = new Uint8Array(32);

const [marketPda, marketBump] = deriveMarketAddress(baseMint, quoteMint, paramsHash);
console.log("market:", marketPda.toBase58(), "bump:", marketBump);

const market = await v2Program.account.market.fetch(marketPda);
console.log("base mint:", market.baseMint.toBase58());
console.log("quote mint:", market.quoteMint.toBase58());
```

## JavaScript runtime-only imports

```javascript
import { IDL, derivePairAddress } from "@omnipair/program-interface";
```

`Omnipair` is a TypeScript type export, not a runtime JavaScript value. In TypeScript, import it with `import type { Omnipair } ...`.

## ESM Compatibility

This package ships strict ESM-compatible output (Node/tsx/bundlers). Relative module specifiers include `.js` extensions in emitted files, so usage works in strict ESM runtimes.

## Exports

### IDL
The Anchor IDL JSON for both Omnipair programs:
- `IDL` - V1 pair program IDL
- `IDL_V2` - V2 market program IDL

### Types
All TypeScript types generated from the IDL:
- `Omnipair` - The program type (type-only export)
- `OmnipairV2` - The V2 program type (type-only export)
- Account types: `Pair`, `UserPosition`, `RateModel`, `FutarchyAuthority`
- Instruction argument types
- Event types

### Constants
- `PROGRAM_ID` / `OMNIPAIR_PROGRAM_ID` - The Omnipair V1 program ID
- `OMNIPAIR_V2_PROGRAM_ID` - The Omnipair V2 program ID
- `SEEDS` - PDA seed constants

### Utilities
- `derivePairAddress(token0, token1, paramsHash)` - Derive a Pair PDA
- `deriveUserPositionAddress(pair, user)` - Derive a UserPosition PDA
- `deriveFutarchyAuthorityAddress()` - Derive FutarchyAuthority PDA
- `deriveReserveVaultAddress(pair, reserveMint)` - Derive a reserve vault PDA
- `deriveCollateralVaultAddress(pair, collateralMint)` - Derive a collateral vault PDA
- `deriveMarketAddress(baseMint, quoteMint, paramsHash)` - Derive a V2 Market PDA
- `deriveMarketV2Address(baseMint, quoteMint, paramsHash)` - Backward-compatible alias for `deriveMarketAddress`
- `deriveMarketReserveVaultAddress(market, reserveMint)` - Derive a V2 market reserve vault PDA
- `deriveMarketCollateralVaultAddress(market, collateralMint)` - Derive a V2 market collateral vault PDA
- `deriveMarketFeeVaultAddress(market, feeMint)` - Derive a V2 market fee vault PDA
- `deriveMarketStakeVaultAddress(market, claimTokenMint)` - Derive a V2 market stake vault PDA
- `deriveStakePositionAddress(market, owner, assetMint)` - Derive a V2 stake position PDA
- `deriveMarginPositionAddress(market, owner)` - Derive a V2 borrower margin position PDA
- `deriveHedgeVaultAddress(market, claimTokenMint)` - Derive a V2 hedged-claim escrow PDA
- `deriveHedgePositionAddress(market, owner, assetMint)` - Derive a V2 hedge position PDA
- `deriveInsuranceReserveAddress(market, assetMint)` - Derive a V2 insurance reserve vault PDA

V2 risk, recognition, and daily-limit books are embedded in the `Market` account rather than standalone PDAs.

## Peer Dependencies

- `@coral-xyz/anchor` >= 0.30.0

## License

MIT
