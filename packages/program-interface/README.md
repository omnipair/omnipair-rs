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

## V2 integration notes

V2 is a separate market program, not a drop-in account rename for the legacy V1 pair program. Integrations should route by program generation and program ID:

- Use `IDL` / `PROGRAM_ID` / `derivePairAddress` for legacy V1 pairs.
- Use `IDL_V2` / `OMNIPAIR_V2_PROGRAM_ID` / `deriveMarketAddress` for V2 markets.
- Store V1 pair metrics and V2 market metrics separately at the source level, then aggregate them under the Omnipair brand in analytics.
- Do not sort V2 market mints client-side. The creator's chosen `baseMint` / `quoteMint` order defines the market and its price direction.
- Treat V2 yLP and hLP mints as distinct Token-2022 token concepts. yLP tokens are floating reserve-side yield shares; hLP tokens are aggregate leveraged LP vault shares. Fee rights are checkpointed through `YieldAccount`.

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
All TypeScript types generated from the IDL, plus named account and event aliases:
- `Omnipair` - The program type (type-only export)
- `OmnipairV2` - The V2 program type (type-only export)
- V1 account types: `Pair`, `UserPosition`, `RateModel`, `FutarchyAuthority`
- V2 account types: `Market`, `MarginPosition`, `YieldAccount`, `V2FutarchyAuthority`
- Instruction argument types
- V2 market/admin event types: `MarketCreated`, `MarketUpdated`, `MarketHealthUpdated`
- V2 liquidity event types: `LiquidityAdded`, `LiquidityRemoved`, `HlpOpened`, `HlpClosed`, `HlpRebalanced`
- V2 lending/settlement event types: `SwapExecuted`, `MarketCollateralDeposited`, `MarketCollateralWithdrawn`, `MarketDebtUpdated`, `PositionLiquidated`
- V2 fee event types: `YieldClaimed`, `YieldRecipientUpdated`, `MarketFeeLiabilityClaimed`, `ProtocolFeesClaimed`

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
- `deriveMarketInterestVaultAddress(market, interestMint)` - Derive a V2 market interest vault PDA
- `deriveMarginPositionAddress(market, owner)` - Derive a V2 borrower margin position PDA
- `deriveYieldAccountAddress(market, owner, assetMint, tokenKind)` - Derive a V2 yLP/hLP revenue checkpoint PDA
- `deriveInsuranceAddress(market, assetMint)` - Derive a V2 insurance vault PDA

V2 risk, debt, yLP share accounting, daily limits, and hLP vault state are embedded in the `Market` account rather than standalone PDAs.

## Peer Dependencies

- `@coral-xyz/anchor` >= 0.30.0

## License

MIT
