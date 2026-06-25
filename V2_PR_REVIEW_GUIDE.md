# Omnipair V2 PR Review Guide

Use this guide to review the V2 market architecture branch without flattening it
into one large diff. V1 compatibility, V2 market terminology, accounting
invariants, generated interfaces, and release gates should each be checked as
separate review tracks.

## Review Entry Points

- `V2_ARCHITECTURE_PLAN.md`: current architecture status and design rationale.
- `V2_PR_BODY.md`: pasteable PR summary, verification summary, review notes,
  and remaining production gates.
- `programs/omnipair-v2/README.md`: V2 source boundaries, integration surface,
  flows, events, invariants, and verification commands.
- `.audit/findings/v2-initial-plan-traceability-2026-06-18.md`: requirement
  traceability against the original V2 plan.
- `.audit/findings/v2-local-readiness-audit-2026-06-18.md`: local build, test,
  artifact, and baseline evidence.
- `.audit/findings/nemesis-v2-final-pass-2026-06-18.md`: final adversarial
  review notes and the remaining non-code decisions.
- `programs/omnipair-v2/RELEASE_CHECKLIST.md`: release, deployment, and
  post-deploy gates.
- `programs/omnipair-v2/SIGNOFF_CHECKLIST.md`: external owner signoffs that are
  still required before production readiness.
- `decoders/omnipair-decoder/README.md`: Carbon decoder usage for legacy V1
  and standalone V2 decoding.

## Suggested Review Order

1. **Program boundary**
   - Confirm `programs/omnipair` remains the legacy V1 pair program.
   - Confirm `programs/omnipair-v2` is a standalone market program with its own
     IDL, program ID, account model, and events.

2. **Public surface**
   - Review the V2 IDL instruction list: `initialize`, `swap`, `add_liquidity`,
     `remove_liquidity`, `stake`, `unstake`, `borrow`, `repay`, `liquidate`,
     hedge, fee, collateral, insurance, and admin flows.
   - Confirm V2 public naming uses `market`, `base`, `quote`, claim tokens,
     hedge tokens, and buffer shares rather than legacy V1 product terminology.

3. **State and accounting**
   - Review `programs/omnipair-v2/src/state`.
   - Focus on `Market`, `MarketSide`, claim-token ledgers, buffer ledgers, fee
     ledgers, debt books, risk books, market health, and position accounts.

4. **Transitions before instructions**
   - Review `programs/omnipair-v2/src/transitions` for atomic accounting
     mutations and receipts.
   - Then review `programs/omnipair-v2/src/instructions` as account validation,
     token movement, slippage checks, and event emission around those
     transitions.

5. **Risk and security fixes**
   - Review cached spot observations, pre-action risk snapshots, liquidity EMA
     daily limits, circuit breakers, normalized debt valuation, health floors,
     config-update safety, liquidation waterfall, and fee-liability settlement.

6. **Deferred scope**
   - Confirm this PR does not enable soft borrow, soft liquidation,
     LLAMMA-style liquidation, Jupiter or external aggregator conversion
     routing, explicit hedge premium pricing, user-selectable settlement side,
     or stale locked collateral-factor machinery.
   - Treat any of those features as requiring a separate reviewed spec rather
     than as follow-on cleanup inside this PR.

7. **Tests and generated interfaces**
   - Review V2 unit/property tests near the state, math, and transition modules.
   - Review LiteSVM flow coverage in `tests/v2-market.test.ts`.
   - Confirm `target/idl/omnipair_v2.json` and `target/types/omnipair_v2.ts`
     match `packages/program-interface/src/idl_v2.json` and
     `packages/program-interface/src/types_v2.ts`.

## Commit Grouping

Do not squash the V2 branch before review. The branch is intentionally split
into logical commit families:

- V1 compatibility and instruction-module separation.
- V2 market state, ledgers, math, transitions, and instruction surface.
- Nemesis/security remediations.
- Modularization and naming polish.
- Tests, generated interfaces, decoder support, and LiteSVM coverage.
- Release, deployment, signoff, and audit documentation.

To review the branch in order, start from the branch base:

```bash
git log --oneline --reverse --no-merges $(git merge-base HEAD main)..HEAD
```

The current history has a few pre-V2 maintenance commits before the V2 work.
For V2-specific review, useful anchors are:

- `a4b8ef6` starts the V1 instruction-module split and compatibility work.
- `5bd547e` starts the original V2 market architecture implementation.
- `028db1e` starts the Nemesis remediation series.
- `92a24ad` starts the move toward the top-level V1/V2 module layout.
- `e09bba0` starts the one-instruction-per-file V2 modularization series.
- `5b9a70c` starts the standalone `programs/omnipair-v2` program shape.
- `bc8e4de` starts the later state/transition/token modularization pass.
- `5b32e1c` starts the V2 decoder support.
- `48f995d` starts the release workflow support for the V2 program.
- `1c0f7e1` starts the readiness/audit/handoff documentation series.

## Local Verification Gates

Run the current V2 gate set before merging or cutting a release candidate:

```bash
cargo fmt -p omnipair-v2 -- --check
cargo check -p omnipair-v2 --lib
cargo test -p omnipair-v2 --lib -- --nocapture
cargo check -p omnipair-v2 --lib --features production
cargo test -p omnipair-v2 --lib --features production -- --nocapture
anchor build -p omnipair-v2
anchor build -p omnipair-v2 -- --features production
npm run build --prefix packages/program-interface
cargo test -p omnipair-decoder --lib
node decoders/omnipair-decoder/scripts/generate-v2-decoder.mjs
yarn test-litesvm
```

Also run the legacy baseline check:

```bash
cargo test -p omnipair --lib
```

The current expected V1 baseline is only the documented five legacy failures in
the V2 readiness audit. Any new V1 failure should be treated as a regression.
The readiness audit also records the latest local snapshots for V2 unit tests,
production-feature tests, LiteSVM, normal and production Anchor builds, package
interface build, decoder test/regeneration, and artifact parity.

## Production Gates

Local tests are not enough to declare V2 production-ready. Before launch,
complete:

- fresh external security review against the final standalone V2 tree;
- app/front-end, SDK, indexer, analytics, aggregator, deployment, and smoke-test
  owner signoffs;
- verifiable production build and Squads deployment flow;
- deployed binary verification with `solana-verify`;
- OtterSec registry submission;
- target-cluster smoke tests.
