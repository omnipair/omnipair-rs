# Omnipair V2 Release Checklist

Use this checklist before treating the standalone V2 market program as
production-ready. The root README covers the shared CI/CD and Squads deployment
mechanics; this file captures the V2-specific gates that must be cleared before
mainnet launch or upgrade.

## 1. Scope Freeze

- Confirm V2 remains a standalone program: `programs/omnipair-v2`.
- Confirm V1 program behavior, instruction names, events, and account layouts
  are unchanged by the V2 release.
- Confirm the emergency reduce-only authority is the intended signer and can
  reach `set_reduce_only` for incident response.
- Confirm soft borrow and soft liquidation remain disabled unless a separate
  reviewed spec has been merged.
- Confirm LLAMMA-style liquidation, Jupiter/external aggregator conversion
  routing, explicit hedge premium pricing, user-selectable settlement side, and
  stale locked collateral-factor machinery remain out of scope unless separate
  reviewed specs have been merged.
- Confirm config updates cannot move existing effective debt below the configured
  market-health floor.

## 2. Security Review

- Run a fresh end-to-end review against the final V2 source tree, not an older
  mixed V1/V2 implementation.
- Re-check the V2 invariants in `programs/omnipair-v2/README.md`.
- Re-check the cached-spot EMA flow for same-slot manipulation resistance.
- Re-check daily-limit enforcement against liquidity EMA for borrow,
  collateral withdrawal, and yLP/hLP settlement paths.
- Re-check liquidation accounting for collateral seizure, insurance draw, and
  LP socialization.
- Re-check fee liabilities: yLP, hLP, operator, protocol, and unallocated
  carry-forward buckets.
- Re-check Token-2022 mint constraints and transfer-fee inventory accounting.

## 3. Local Verification

Run these gates from the repository root:

```bash
cargo fmt -p omnipair-v2 -- --check
cargo check -p omnipair-v2 --lib
cargo test -p omnipair-v2 --lib -- --nocapture
cargo check -p omnipair-v2 --lib --features production
cargo test -p omnipair-v2 --lib --features production -- --nocapture
anchor build -p omnipair_v2
anchor build -p omnipair_v2 -- --features production
npm run build --prefix packages/program-interface
cargo test -p omnipair-decoder --lib
node decoders/omnipair-decoder/scripts/generate-v2-decoder.mjs
yarn test-litesvm
```

Also run the V1 baseline check and confirm it has only the documented legacy
failures:

```bash
cargo test -p omnipair --lib
```

## 4. Artifact Review

- Confirm `target/idl/omnipair_v2.json` exists and matches the intended public
  V2 surface.
- Confirm `target/types/omnipair_v2.ts` exists and matches the same build.
- Confirm `packages/program-interface/src/idl_v2.json` and
  `packages/program-interface/src/types_v2.ts` were regenerated from that build
  if any public IDL shape changed.
- Confirm `packages/program-interface/src/constants.ts` exports the intended V2
  program ID and PDA helpers.
- Confirm yLP and hLP Token-2022 mint constraints remain represented in both
  code and IDL-visible account flows.

## 5. Integration Readiness

- Complete the owner signoff register in
  `programs/omnipair-v2/SIGNOFF_CHECKLIST.md`.
- Review the integrator handoff in `programs/omnipair-v2/README.md` with app,
  SDK, indexer, analytics, and aggregator owners.
- SDK consumers use `IDL_V2`, `OmnipairV2`, and `OMNIPAIR_V2_PROGRAM_ID`.
- Market PDA derivation uses `deriveMarketAddress` or `deriveMarketV2Address`.
- Indexers consume V2 events from the standalone V2 IDL and do not reuse V1
  pair event decoders.
- App routing points new market flows at V2 while keeping legacy V1 access
  available.
- Analytics distinguish V1 pair liquidity from V2 market yLP, hLP, debt,
  insurance, and fee state.

## 6. Mainnet Deployment

- Confirm `programs/omnipair-v2/src/lib.rs` declares the intended program ID.
- Build the verifiable V2 binary with production features:

```bash
export GIT_REV=$(git rev-parse HEAD)
export GIT_RELEASE=$(git describe --tags --abbrev=0 2>/dev/null || echo "dev")

anchor build --verifiable -p omnipair_v2 \
  -e GIT_REV=$GIT_REV \
  -e GIT_RELEASE=$GIT_RELEASE \
  -- --features "production"
```

- Confirm the release contains:

```text
target/verifiable/omnipair_v2.so
target/idl/omnipair_v2.json
target/types/omnipair_v2.ts
```

- Deploy the upgrade buffer through the documented workflow with `program=v2`.
- Transfer upgrade buffer authority to the configured Squads vault.
- Create and approve the Squads upgrade proposal for the V2 program ID.

## 7. Post-Deploy Verification

- Verify the deployed V2 binary with `solana-verify`.
- Use trailing cargo args for production verification:
  `-- --features production --config "env.GIT_REV=\"...\"" --config "env.GIT_RELEASE=\"...\""`.
- Submit the verified V2 build to the OtterSec registry.
- Publish `@omnipair/program-interface` only after the verified IDL and types
  match the deployed binary.
- Confirm the app, SDK, and indexers are using the deployed V2 program ID.
- Smoke-test market initialization, add/remove liquidity, swap, borrow/repay,
  liquidation rejection while healthy, yield claims, protocol fee claims, and
  hLP open/close on the target cluster.
