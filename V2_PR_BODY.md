# Omnipair V2 Market Architecture

Suggested PR title: `feat(v2): add standalone market architecture`

## Summary

This PR adds Omnipair V2 as a standalone market architecture program while
keeping the legacy V1 pair program available and compatible.

V2 is not a V1 account rename. It has its own program ID, IDL, market accounts,
yLP and hLP token semantics, risk books, events, generated
interfaces, decoder support, release checklist, and owner signoff register.

## What Changed

- Added `programs/omnipair-v2` as the standalone V2 Anchor program.
- Kept `programs/omnipair` as the legacy V1 pair program.
- Added clean V2 instruction names such as `initialize`, `swap`,
  `add_liquidity`, `remove_liquidity`, `borrow`, `repay`, `liquidate`,
  `stake`, `claim_fees`, `open_hedge`, and `close_hedge`.
- Added V2 market state, ledgers, risk books, health accounting, positions,
  token semantics modules, transition modules, events, errors, seeds, and SDK
  PDA helpers.
- Added package-interface V2 IDL/types and named V2 account/event aliases.
- Added Carbon decoder V2 generation and decoder tests.
- Updated release workflow/docs for V2 verifiable builds, artifacts, manual
  deploy/verify paths, package publishing, and decoder regeneration.

## Core V2 Invariants

- yLP tokens are the normal two-sided LP shares.
- Deposits split into protected claim amount plus retained buffer shares.
- Fee rights require matched staked claim tokens and buffer shares.
- Fees are explicit non-compounding liabilities settled through fee ledgers.
- Reserve floors protect claim supply plus required buffer on each market side.
- Market health uses recognized debt-bearing collateral only.
- Fixed debt is valued in normalized market units; hedged overlay debt is
  gamma-weighted against liquidity EMA.
- Cached spot observations feed EMA updates instead of same-instruction
  post-state spot.
- Liquidation follows borrower collateral, liquidator repayment/incentive,
  insurance reserve, then LP socialization.
- hLP tokens are one-sided hedged LP shares backed by aggregate vault-owned yLP.

## Security And Risk Work

The branch includes the verified Nemesis remediation set:

- value V2 health/liquidation debt with risk-book pricing instead of raw token
  units;
- recompute buffer floors on config updates;
- lock buffer-ratio changes while active stake or staker fee liabilities exist;
- enforce liquidity-EMA daily limits and circuit breakers;
- settle operator/protocol/unallocated fee liabilities.

Additional hardening includes cached spot observations, pre-action risk
snapshots, risk checks on settlement paths, Token-2022 inventory-credit
handling, reduce-only emergency authority, and V2 security metadata that no
longer self-reports legacy V1 auditors.

## Verification

Latest local evidence is recorded in
`.audit/findings/v2-local-readiness-audit-2026-06-18.md`.

The documented local gate set includes:

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
cargo test -p omnipair --lib
```

Expected current local results:

- V2 unit/property tests pass with 94 tests.
- V2 production-feature tests pass with 94 tests.
- LiteSVM passes with 42 tests and V2 instruction smoke coverage `19/19`.
- V2 normal and production Anchor builds pass with known SBF/LTO/linkage
  warnings.
- `@omnipair/program-interface` builds successfully.
- V2 IDL/types match package-interface copies byte-for-byte.
- V2 Carbon decoder tests pass and V2 decoder regeneration produces no tracked
  changes.
- V1 baseline remains the documented five legacy failures only.

Known V1 baseline failures:

- `v1::state::rate_model::tests::test_default_matches_original_high_util`
- `v1::state::rate_model::tests::test_default_matches_original_low_util`
- `v1::state::rate_model::tests::test_faster_half_life_adjusts_quicker`
- `v1::state::rate_model::tests::test_uncapped_rate_grows_exponentially`
- `shared::gamm_math::tests::manipulation_bounded_by_ema`

## Deferred Feature Scope

These original V2 ideas are intentionally not enabled in this PR:

- soft borrow and soft liquidation;
- LLAMMA-style soft liquidation;
- Jupiter or external aggregator conversion routing;
- explicit hedge premium pricing;
- user-selectable settlement side;
- stale locked collateral-factor machinery.

Current V2 uses fixed-token debt, inventory-native settlement, recognized
collateral valuation, and hLP shares with routed hedge fees. Any deferred
feature above should land behind a separate reviewed spec.

## Review Notes

- Do not squash before review; the branch is intentionally split into logical
  commit families for architecture, remediations, modularization, tests,
  generated interfaces, decoder support, and release readiness.
- Start with `V2_PR_REVIEW_GUIDE.md`, then use
  `V2_ARCHITECTURE_PLAN.md`, `programs/omnipair-v2/README.md`, and the audit
  files for deeper review.
- The deferred feature scope above remains intentionally disabled until
  separate reviewed specs are merged.

## Remaining Production Gates

Local tests do not make V2 production-ready. Before launch, complete:

- fresh external security review against the final standalone V2 source tree;
- app/front-end, SDK, indexer, analytics, aggregator, deployment, and
  smoke-test owner signoffs;
- verifiable production build and Squads deployment flow;
- deployed binary verification with `solana-verify`;
- OtterSec registry submission;
- target-cluster smoke tests after deployment.
