# V2 Initial Plan Traceability - 2026-06-18

## Scope

- Branch: `feat/v2-market-architecture`
- V2 program: `programs/omnipair-v2`
- Legacy V1 program: `programs/omnipair`
- Purpose: map the original V2 market architecture plan to current local
  evidence, separating implemented protocol surface from deferred design items
  and external release gates.

## Current V2 Surface

The current standalone V2 IDL exposes 19 instructions:

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

The current standalone V2 IDL exposes four account types:

```text
HedgePosition
MarginPosition
Market
StakePosition
```

The current standalone V2 IDL exposes product-facing market events such as
`LiquidityAdded`, `LiquidityRemoved`, `SwapExecuted`, `PositionLiquidated`,
`MarketHealthUpdated`, `MarketFeesClaimed`, and `MarketHedgeFeesClaimed`.

## Requirement Traceability

| Original requirement | Current status | Evidence |
| --- | --- | --- |
| Keep V1 compatible and available. | Implemented locally. | V1 remains in `programs/omnipair`; V2 is standalone in `programs/omnipair-v2`; V2 docs and package interface describe separate V1/V2 routing. |
| Add V2 without mixed V1/V2 instruction roots. | Implemented locally, with intentional architecture change from the earliest same-program assumption. | `V2_ARCHITECTURE_PLAN.md` records the separate-program decision; `programs/omnipair-v2/src/lib.rs` exposes the standalone V2 program. |
| Use market terminology and avoid V2 `pair` / `pool` public concepts. | Implemented locally for V2 public surface. | V2 IDL account type is `Market`; PDA seed is `market_v2`; V2 docs use market wording. Legacy V1 docs can still say pair/pool. |
| Use clean V2 instruction names rather than `v2_*` or `market_*` prefixes. | Implemented locally after the separate-program decision. | V2 IDL exposes `swap`, `borrow`, `repay`, `liquidate`, `add_liquidity`, and related action names. |
| Keep V1-style modular instruction layout. | Implemented locally. | `programs/omnipair-v2/src/instructions` is split by domain with one instruction per file, plus small domain `common.rs` files. |
| Add market state, vaults, seeds, events, errors, and SDK helpers. | Implemented locally. | `programs/omnipair-v2/src/state`, `events.rs`, `errors.rs`, `constants.rs`, and `packages/program-interface/src/constants.ts`. |
| Use precise financial/accounting vocabulary for retained junior risk capital. | Implemented locally. | `buffer shares` is the explicit V2 term in state, events, IDL/types, README, and naming plan; no separate branded label is introduced at the protocol boundary. |
| Implement claim-minus-buffer deposits. | Implemented locally. | `transitions/reserve.rs` uses `split_claim_minus_buffer`; tests cover `add_liquidity_mints_claim_minus_buffer`. |
| Keep base `omLP` fixed 1:1 principal, no rebase or dynamic exchange rate. | Implemented locally. | `remove_liquidity` burns claim tokens for fixed principal; V2 README lists fixed-principal claim tokens as a core invariant. |
| Require matched staking for fee rights. | Implemented locally. | `StakePosition` tracks staked claim and buffer amounts; `active_stake_units` gates fee allocation. |
| Route fees as non-compounding liabilities and fee-growth indexes. | Implemented locally. | `FeeLedger` carries staker, hedge, operator, protocol, and unallocated liabilities; fee claim instructions settle explicit buckets. |
| Enforce reserve floors on swaps and withdrawals. | Implemented locally. | `transitions/swap.rs` and `transitions/reserve.rs` require reserve floors covering protected claims plus required buffer. |
| Default to fixed-token debt. | Implemented locally. | `borrow`/`repay` operate on fixed debt shares; `soft_borrow_enabled` is rejected in `MarketConfig::validate`. |
| Use recognized debt-bearing collateral only for health. | Implemented locally. | `RecognitionLedger` and `MarginPosition` separate idle collateral from recognized collateral; tests cover idle-collateral pump rejection. |
| Value health/liquidation in normalized market units rather than raw token units. | Implemented locally after Nemesis remediation. | `state/health.rs` normalizes values and uses risk-book prices; tests cover raw-unit decimal pump rejection and liquidation consistency. |
| Cache spot observations for EMA updates to avoid same-instruction manipulation. | Implemented locally. | `RiskBook` stores `cached_spot_base_price_nad` and `cached_spot_quote_price_nad`; swap/add-liquidity tests cover pre-action snapshots. |
| Enforce liquidity-EMA daily limits and circuit breakers. | Implemented locally. | `DailyLimitBook`, side liquidity EMA fields, `enforce_daily_borrow_limit`, `enforce_daily_withdraw_limit`, and risk circuit-breaker tests. |
| Recompute buffer floors and lock fee indexes across buffer-ratio changes. | Implemented locally. | `apply_buffer_ratio_update` recomputes floors and rejects active stake or staker fee liabilities. |
| Settle operator/protocol/unallocated fee liabilities. | Implemented locally. | `claim_market_fees`, fee carry-forward helpers, and fee-ledger tests cover liability settlement. |
| Liquidate via borrower collateral, liquidator repayment, insurance, then LP socialization. | Implemented locally. | `transitions/liquidation.rs`, `deposit_insurance`, `PositionLiquidated`, and LiteSVM liquidation flows. |
| Implement h-omLP wrappers as dynamic overlays that unwrap to base omLP without staking rights. | Implemented locally. | `open_hedge`, `claim_hedge_fees`, `close_hedge`, `HedgePosition`, and hedge tests cover 1:1 wrapper behavior. |
| Use inventory-native settlement. | Implemented locally. | Instruction handlers measure actual token-account credits/debits for reserve, collateral, fee, insurance, claim, and hedge vault flows. |
| Update generated TypeScript/IDL helpers. | Implemented locally. | `packages/program-interface/src/idl_v2.json`, `types_v2.ts`, and PDA helpers are checked against `target/idl` and `target/types` in the readiness audit. |
| Keep V2 work reviewable in logical commits. | Implemented in current branch history. | The branch contains separate `feat`, `fix`, `refactor`, `test`, `chore`, and `docs` commits for V2 architecture, Nemesis remediations, modularization, generated interfaces, and release docs. |

## Deferred By Design

These items are not missing from the current implementation because the plan
explicitly deferred or disabled them:

| Item | Current handling |
| --- | --- |
| Soft borrow and soft liquidation | `soft_borrow_enabled` is rejected by config validation. A separate reviewed spec is required before enabling. |
| Jupiter conversion / aggregator conversion path | Deferred; V2 exposes its own swap venue and integrator handoff. |
| LLAMMA-style soft liquidation | Deferred with soft liquidation. |
| Explicit hedge premium | Deferred; current h-omLP wrapper tracks hedged exposure and routed hedge fees. |
| User-selectable settlement side | Deferred; current flows use inventory-native settlement. |
| Stale locked collateral-factor machinery | Deferred; current V2 health uses recognized collateral and risk-book valuation. |

## External Gates

These cannot be completed purely from local code inspection and remain release
blockers before production readiness:

- fresh end-to-end security review against the final standalone V2 source tree;
- app/front-end owner signoff for V2 routing and legacy V1 access;
- SDK, indexer, analytics, and aggregator owner signoff against the V2 handoff;
- mainnet deployment and Squads upgrade checklist execution;
- deployed-binary verification with `solana-verify` and OtterSec registry
  submission;
- target-cluster smoke tests after deployment.

The owner signoff register for these gates is
`programs/omnipair-v2/SIGNOFF_CHECKLIST.md`.

## Local Verification Evidence

Recent local evidence is recorded in
`.audit/findings/v2-local-readiness-audit-2026-06-18.md`, including:

- `cargo check -p omnipair-v2 --lib`;
- `cargo test -p omnipair-v2 --lib -- --nocapture` with 94 tests;
- `anchor build -p omnipair-v2`;
- production-feature `cargo check`, `cargo test`, and `anchor build`;
- `yarn test-litesvm` with V2 instruction smoke coverage `19/19`;
- `npm run build --prefix packages/program-interface`;
- `cargo test -p omnipair-decoder --lib`;
- V2 decoder regeneration from `packages/program-interface/src/idl_v2.json`;
- V2 IDL/type artifact equality checks;
- V1 baseline check with only the documented five legacy failures.

Current traceability refresh also rechecked:

- standalone V2 IDL instruction/account/event names from
  `packages/program-interface/src/idl_v2.json`;
- V2 instruction file layout under `programs/omnipair-v2/src/instructions`;
- V2 source and generated V2 artifacts have no legacy V1 product-terminology
  leftovers in the V2 public surface;
- decoder regeneration produces no tracked V2 decoder or artifact changes;
- `buffer shares` is the recorded protocol term for retained junior
  risk-capital accounting;
- absence of non-test `.unwrap()`, `panic!`, and `unimplemented!` in
  `programs/omnipair-v2/src`.
