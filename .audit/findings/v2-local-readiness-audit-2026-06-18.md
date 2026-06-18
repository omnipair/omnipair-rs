# V2 Local Readiness Audit - 2026-06-18

## Scope

- Branch: `feat/v2-market-architecture`
- Program: `programs/omnipair-v2`
- Legacy program: `programs/omnipair`
- Goal: verify the current local V2 implementation against the V2 market
  architecture plan and separate local evidence from external release gates.

## Implemented Local Surface

The standalone V2 IDL exposes 19 instructions:

```text
add_liquidity
borrow
claim_fees
claim_hedge_fees
claim_market_fees
close_hedge
deposit_collateral
deposit_insurance
initialize
liquidate
open_hedge
remove_liquidity
repay
set_reduce_only
stake
swap
unstake
update_config
withdraw_collateral
```

The standalone V2 IDL exposes these account types:

```text
HedgePosition
MarginPosition
Market
StakePosition
```

The standalone V2 IDL exposes these event types:

```text
LiquidityAdded
LiquidityRemoved
MarketCollateralDeposited
MarketCollateralWithdrawn
MarketCreated
MarketDebtUpdated
MarketFeeLiabilityClaimed
MarketFeesClaimed
MarketHealthUpdated
MarketHedgeClosed
MarketHedgeFeesClaimed
MarketHedgeOpened
MarketInsuranceFunded
MarketStakeUpdated
MarketUpdated
PositionLiquidated
SwapExecuted
```

## Local Evidence

Recent local verification covered:

| Requirement | Evidence |
| --- | --- |
| V2 program builds and type-checks | `cargo check -p omnipair-v2 --lib` passed. |
| V2 unit/property coverage passes | `cargo test -p omnipair-v2 --lib -- --nocapture` passed with 94 tests. |
| V2 Anchor artifact builds | `anchor build -p omnipair-v2` passed, with known SBF/linkage warnings. |
| V2 production feature type-checks | `cargo check -p omnipair-v2 --lib --features production` passed. |
| V2 production feature tests pass | `cargo test -p omnipair-v2 --lib --features production -- --nocapture` passed with 94 tests. |
| V2 production Anchor artifact builds | `anchor build -p omnipair-v2 -- --features production` passed, with known SBF/linkage warnings. |
| V2 LiteSVM flows cover all public instructions | `yarn test-litesvm` passed with 42 tests and V2 instruction smoke coverage `19/19`. |
| Package interface builds | `npm run build --prefix packages/program-interface` passed. |
| V2 decoder compiles | `cargo test -p omnipair-decoder --lib` passed with 1 decoder test. |
| V2 IDL package copy matches build artifact | `target/idl/omnipair_v2.json` equals `packages/program-interface/src/idl_v2.json`. |
| V2 TypeScript package copy matches build artifact | `target/types/omnipair_v2.ts` equals `packages/program-interface/src/types_v2.ts`. |
| V1 baseline is unchanged | `cargo test -p omnipair --lib` fails only on the documented 5 legacy failures. |

Current-head refresh at `3f23e7d` re-ran the documented local gates:

| Gate | Result |
| --- | --- |
| `cargo fmt -p omnipair-v2 -- --check` | Passed. |
| `cargo check -p omnipair-v2 --lib` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `cargo test -p omnipair-v2 --lib -- --nocapture` | Passed with 94 tests. |
| `anchor build -p omnipair-v2` | Passed with known SBF/linkage warnings. |
| `cargo check -p omnipair-v2 --lib --features production` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `cargo test -p omnipair-v2 --lib --features production -- --nocapture` | Passed with 94 tests. |
| `anchor build -p omnipair-v2 -- --features production` | Passed with known SBF/linkage warnings. |
| `npm run build --prefix packages/program-interface` | Passed. |
| `yarn test-litesvm` | Passed with 42 tests and V2 instruction smoke coverage `19/19`. |
| V2 IDL and TypeScript artifact equality | `target/idl/omnipair_v2.json` and `target/types/omnipair_v2.ts` match the package copies. |
| `cargo test -p omnipair --lib` | Failed only on the documented five legacy V1 failures. |

Latest docs-only readiness refresh at `d73c61b` did not change V2 code or
generated artifacts. It rechecked and documented:

- owner signoff tracking in `programs/omnipair-v2/SIGNOFF_CHECKLIST.md`;
- absence of legacy V1 product-terminology leftovers in V2 source and generated
  V2 artifacts;
- `buffer shares` as the explicit term for retained junior risk-capital
  accounting;
- product-facing V2 event naming as the current completed choice.

## Local Completion Notes

- V2 is a standalone program, not a versioned instruction set inside V1.
- V1 public pair naming and behavior remain in the legacy program.
- V2 public instruction names are clean action names, not `v2_*` or
  `market_*` workaround names.
- V2 code keeps V1-style one-instruction-per-file domain folders.
- V2 source and generated V2 artifacts do not expose legacy V1 product
  terminology in the V2 public surface.
- `buffer shares` remains the explicit V2 term for retained junior
  risk-capital accounting.
- Soft borrow and soft liquidation remain intentionally disabled until a
  separate reviewed spec is merged.
- App, SDK, indexer, analytics, and aggregator handoff notes are documented in
  `programs/omnipair-v2/README.md`.
- External owner signoffs are tracked in
  `programs/omnipair-v2/SIGNOFF_CHECKLIST.md`.

## Remaining External Gates

These are not proven by local tests and still require owner or deployment
process signoff:

- fresh end-to-end security review against the final standalone V2 source tree;
- app/front-end routing owner signoff;
- SDK/indexer/analytics/aggregator owner signoff against the V2 handoff;
- mainnet deployment and Squads upgrade checklist execution;
- deployed-binary verification with `solana-verify` and OtterSec submission;
- target-cluster smoke tests after deployment.
