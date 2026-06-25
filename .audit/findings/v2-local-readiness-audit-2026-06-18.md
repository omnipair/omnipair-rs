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

Earlier code-gate refresh at `3f23e7d` re-ran the documented local gates:

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

Docs-only readiness refreshes through `6cffae5` did not change V2 code or
generated artifacts. They rechecked and documented:

- owner signoff tracking in `programs/omnipair-v2/SIGNOFF_CHECKLIST.md`;
- absence of legacy V1 product-terminology leftovers in V2 source and generated
  V2 artifacts;
- `buffer shares` as the explicit term for retained junior risk-capital
  accounting;
- product-facing V2 event naming as the current completed choice;
- the V2 PR review guide in `V2_PR_REVIEW_GUIDE.md`.

Verification refresh at `6cffae5` re-ran the local review gates:

| Gate | Result |
| --- | --- |
| `git diff --check` | Passed. |
| V2 IDL and TypeScript artifact equality | `target/idl/omnipair_v2.json` and `target/types/omnipair_v2.ts` match the package copies. |
| `cargo fmt -p omnipair-v2 -- --check` | Passed. |
| `cargo check -p omnipair-v2 --lib` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `cargo test -p omnipair-v2 --lib -- --nocapture` | Passed with 94 tests. |
| `cargo check -p omnipair-v2 --lib --features production` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `cargo test -p omnipair-v2 --lib --features production -- --nocapture` | Passed with 94 tests. |
| `anchor build -p omnipair-v2` | Passed with known SBF/linkage warnings. |
| `anchor build -p omnipair-v2 -- --features production` | Passed with known SBF/linkage warnings. |
| `npm run build --prefix packages/program-interface` | Passed. |
| `yarn test-litesvm` | Passed with 42 tests and V2 instruction smoke coverage `19/19`. |
| `cargo test -p omnipair --lib` | Failed only on the documented five legacy V1 failures. |

The V1 baseline run generated a transient
`programs/omnipair/proptest-regressions/` artifact; it was removed after
confirming the failure set matched the documented baseline.

Follow-up evidence at `95d347c`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-decoder --lib` | Passed with 1 decoder test. |
| V2 production panic/placeholder scan | A cfg-test-aware scan found no production `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` hits under `programs/omnipair-v2/src`. |

Follow-up release workflow sanity check at `c278095`:

| Gate | Result |
| --- | --- |
| `.github/workflows/release-build.yaml` YAML parse | Passed with Ruby/Psych `YAML.load_file`. |
| V2 release workflow path inspection | The workflow includes V2 verifiable build, required V2 release artifacts, manual `program=v2` buffer deployment, V2 `solana-verify` library selection, V2 package artifact download, and decoder publish regeneration from `omnipair_v2.json`. This is local workflow inspection, not a live GitHub Actions run. |

Follow-up V2 security metadata alignment:

| Gate | Result |
| --- | --- |
| V2 `security_txt` auditor metadata | V2 no longer self-reports legacy V1 auditors. The V2 auditor field now records that the final V2 security review is pending, matching the release/signoff docs. |
| Root audit wording | The root README now scopes Offside Labs and Ackee audit wording to legacy V1 code and shared protocol components, while pointing V2 to the pending signoff checklist. |
| `cargo fmt -p omnipair-v2 -- --check` | Passed. |
| `cargo check -p omnipair-v2 --lib` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `cargo check -p omnipair-v2 --lib --features production` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |

Follow-up V2 production artifact check at `afe1e16`:

| Gate | Result |
| --- | --- |
| `anchor build -p omnipair-v2 -- --features production` | Passed with known SBF/LTO/linkage warnings. The build embedded `GIT_REV=afe1e16d40922703a96992a8ef14fb0b96415b35` and `GIT_RELEASE=v0.10.2`. |
| Built V2 `security_txt` string check | `strings target/deploy/omnipair_v2.so` contains `Pending final V2 security review`, confirming the V2 auditor metadata made it into the built artifact. |
| Generated V2 IDL/types after build | No tracked generated V2 IDL or TypeScript artifacts changed after the production Anchor build. |

Follow-up V2 artifact/package/decoder parity at `c56facf`:

| Gate | Result |
| --- | --- |
| V2 IDL package parity | `target/idl/omnipair_v2.json` matches `packages/program-interface/src/idl_v2.json` byte-for-byte. |
| V2 TypeScript package parity | `target/types/omnipair_v2.ts` matches `packages/program-interface/src/types_v2.ts` byte-for-byte. |
| V2 IDL surface count | The current V2 IDL has 19 instructions, 4 account types, and 17 event types. |
| V2 decoder regeneration | `node decoders/omnipair-decoder/scripts/generate-v2-decoder.mjs` completed from `packages/program-interface/src/idl_v2.json` and produced no tracked decoder changes. |
| Package interface build | `npm run build --prefix packages/program-interface` passed. |
| V2 decoder test | `cargo test -p omnipair-decoder --lib` passed with 1 decoder test. |

Behavior refresh at `beec854`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-v2 --lib -- --nocapture` | Passed with 94 tests and only the known Anchor macro `unexpected cfg solana` warnings. |
| `yarn test-litesvm` | Passed with 42 tests, V2 instruction smoke coverage `19/19`, and legacy V1 smoke coverage unchanged at `2/21`. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/` or `programs/omnipair/proptest-regressions/` directories were created by this refresh. |

Production feature refresh at `7b1ac8b`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-v2 --lib --features production -- --nocapture` | Passed with 94 tests and only the known Anchor macro `unexpected cfg solana` warnings. The test build embedded `GIT_REV=7b1ac8b65598111c9585dfd402a552396755ad76` and `GIT_RELEASE=v0.10.2`. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/` or `programs/omnipair/proptest-regressions/` directories were created by this refresh. |

V1 baseline refresh at `4b8e599`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair --lib` | Failed only on the documented five legacy V1 failures: `v1::state::rate_model::tests::test_default_matches_original_high_util`, `v1::state::rate_model::tests::test_default_matches_original_low_util`, `v1::state::rate_model::tests::test_faster_half_life_adjusts_quicker`, `v1::state::rate_model::tests::test_uncapped_rate_grows_exponentially`, and `shared::gamm_math::tests::manipulation_bounded_by_ema`. The run passed 50 tests and failed 5. |
| Transient proptest artifacts | The run generated `programs/omnipair/proptest-regressions/shared/gamm_math.txt`; it was removed after confirming the failure set matched the documented baseline. |

Production artifact rebuild at `cf4a8ee`:

| Gate | Result |
| --- | --- |
| `anchor build -p omnipair-v2 -- --features production` | Passed with known SBF/LTO/linkage warnings. The build output embedded `GIT_REV=cf4a8eeaa5766368bfa0b5e793e62176cfc04f2a` and `GIT_RELEASE=v0.10.2`. |
| Built V2 `security_txt` string check | `strings target/deploy/omnipair_v2.so` contains `Pending final V2 security review` and does not show legacy V1 auditor names. |
| V2 IDL/type package parity after rebuild | `target/idl/omnipair_v2.json` matches `packages/program-interface/src/idl_v2.json`; `target/types/omnipair_v2.ts` matches `packages/program-interface/src/types_v2.ts`. |

Local gap audit at `9e17c7a`:

| Gate | Result |
| --- | --- |
| V2 production placeholder scan | A cfg-test-aware scan found no production `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` hits under `programs/omnipair-v2/src`. |
| V2 source/docs TODO scan | No V2 source `TODO`/`FIXME`/placeholder items were found. The remaining pending/TBD hits are the owner signoff register, external deployment/verification gates, and intentionally deferred feature scope. |
| Remaining locally actionable implementation gaps | None found in this pass. Production readiness still depends on the external gates listed below. |

Post-gap docs-only handoff refresh:

| Gate | Result |
| --- | --- |
| Post-gap changed-file scope | Changes after `9e17c7a` are limited to V2 handoff, README, release/signoff, and audit documentation. No `programs/omnipair-v2/src`, generated IDL/type, decoder, or test files changed in this docs-only refresh range. |
| Deferred-scope visibility | `V2_ARCHITECTURE_PLAN.md`, `V2_PR_BODY.md`, `V2_PR_REVIEW_GUIDE.md`, `programs/omnipair-v2/README.md`, `programs/omnipair-v2/RELEASE_CHECKLIST.md`, and `programs/omnipair-v2/SIGNOFF_CHECKLIST.md` now explicitly gate soft borrow/liquidation, LLAMMA-style liquidation, Jupiter or external aggregator conversion routing, explicit hedge premium pricing, user-selectable settlement side, and stale locked collateral-factor machinery as separate-spec work. |

Lightweight source/interface verification snapshot at `159e8d1`:

| Gate | Result |
| --- | --- |
| V2 source and generated artifact tracked diff | No tracked diffs under `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types. |
| V2 IDL/type package parity | `target/idl/omnipair_v2.json` matches `packages/program-interface/src/idl_v2.json`; `target/types/omnipair_v2.ts` matches `packages/program-interface/src/types_v2.ts`. |
| `cargo fmt -p omnipair-v2 -- --check` | Passed. |
| `cargo check -p omnipair-v2 --lib` | Passed with the known Anchor macro `unexpected cfg solana` warnings. |
| `npm run build --prefix packages/program-interface` | Passed and produced no tracked package-interface diffs. |

V2 behavior verification snapshot at `9adeb95`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-v2 --lib -- --nocapture` | Passed with 94 tests and only the known Anchor macro `unexpected cfg solana` warnings. The test build embedded `GIT_REV=9adeb950f7fc20f5355875a91c86c103dc143b3f` and `GIT_RELEASE=v0.10.2`. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/` or `programs/omnipair/proptest-regressions/` files were created by this refresh. |
| V2 source and generated artifact tracked diff | No tracked diffs under `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types after the test run. |

V2 production-feature behavior verification snapshot at `d3d6aa4`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-v2 --lib --features production -- --nocapture` | Passed with 94 tests and only the known Anchor macro `unexpected cfg solana` warnings. The test build embedded `GIT_REV=d3d6aa457e912bbcb3cc54959366edccee8d70a1` and `GIT_RELEASE=v0.10.2`. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/` or `programs/omnipair/proptest-regressions/` files were created by this refresh. |
| V2 source and generated artifact tracked diff | No tracked diffs under `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types after the production-feature test run. |

LiteSVM flow verification snapshot at `57553e6`:

| Gate | Result |
| --- | --- |
| `yarn test-litesvm` | Passed with 42 tests. V2 instruction smoke coverage was `19/19` (`100.00%`), legacy V1 smoke coverage remained `2/21` (`9.52%`), and aggregate smoke coverage was `21/40` (`52.50%`). |
| Tracked test and generated artifact diff | No tracked diffs under `tests`, package V2 IDL/types, or target V2 IDL/types after the LiteSVM run. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/` or `programs/omnipair/proptest-regressions/` files were present after the LiteSVM run. |

Anchor build verification snapshot at `c740fdb`:

| Gate | Result |
| --- | --- |
| `anchor build -p omnipair-v2` | Passed with known SBF/LTO/linkage warnings. The build embedded `GIT_REV=c740fdb1340365e1834e8518ee97bdf49045d2e8` and `GIT_RELEASE=v0.10.2`. |
| Built V2 `security_txt` string check | `strings target/deploy/omnipair_v2.so` contains `Pending final V2 security review` and did not show legacy V1 auditor names in the checked output. |
| V2 IDL/type package parity after build | `target/idl/omnipair_v2.json` matches `packages/program-interface/src/idl_v2.json`; `target/types/omnipair_v2.ts` matches `packages/program-interface/src/types_v2.ts`. |
| V2 source and generated artifact tracked diff | No tracked diffs under `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types after the Anchor build. |

Production Anchor build verification snapshot at `dd0d17d`:

| Gate | Result |
| --- | --- |
| `anchor build -p omnipair-v2 -- --features production` | Passed with known SBF/LTO/linkage warnings. The build embedded `GIT_REV=dd0d17d279702819a1dbb857b9293b28d76b0ed3` and `GIT_RELEASE=v0.10.2`. |
| Built V2 `security_txt` and release metadata string check | `strings target/deploy/omnipair_v2.so` contains `Pending final V2 security review`, `dd0d17d279702819a1dbb857b9293b28d76b0ed3`, and `v0.10.2`. The checked output did not show legacy V1 auditor names. |
| V2 IDL/type package parity after production build | `target/idl/omnipair_v2.json` matches `packages/program-interface/src/idl_v2.json`; `target/types/omnipair_v2.ts` matches `packages/program-interface/src/types_v2.ts`. |
| V2 source and generated artifact tracked diff | No tracked diffs under `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types after the production Anchor build. |

Decoder verification snapshot at `ad033b6`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair-decoder --lib` | Passed with 1 decoder test. |
| V2 decoder regeneration | `node decoders/omnipair-decoder/scripts/generate-v2-decoder.mjs` completed from `packages/program-interface/src/idl_v2.json` and produced no tracked decoder or V2 artifact changes. |
| Transient proptest artifacts | No `programs/omnipair-v2/proptest-regressions/`, `programs/omnipair/proptest-regressions/`, or decoder proptest-regression files were present after the decoder refresh. |

Legacy V1 baseline snapshot at `db83f5b`:

| Gate | Result |
| --- | --- |
| `cargo test -p omnipair --lib` | Failed only on the documented five legacy V1 failures: `v1::state::rate_model::tests::test_default_matches_original_high_util`, `v1::state::rate_model::tests::test_default_matches_original_low_util`, `v1::state::rate_model::tests::test_faster_half_life_adjusts_quicker`, `v1::state::rate_model::tests::test_uncapped_rate_grows_exponentially`, and `shared::gamm_math::tests::manipulation_bounded_by_ema`. The run passed 50 tests and failed 5, with only the known Anchor macro `unexpected cfg solana` warnings. |
| Transient proptest artifacts | The run generated `programs/omnipair/proptest-regressions/shared/gamm_math.txt`; it was removed after confirming the failure set matched the documented baseline. |

Current local-gap scan snapshot at `828a1f8`:

| Gate | Result |
| --- | --- |
| V2 cfg-test-aware production placeholder scan | No production-path `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` hits were found under `programs/omnipair-v2/src`. Inline Rust test modules were excluded from this scan. |
| V2 TODO/FIXME/TBD scan | Hits are limited to the explicit pending owner signoff rows in `programs/omnipair-v2/SIGNOFF_CHECKLIST.md` and one V1 non-goal sentence in `V2_ARCHITECTURE_PLAN.md`. No new V2 source TODO/FIXME items were found. |
| V2 public terminology scan | No `pair`/`pool` terminology hits were found in `programs/omnipair-v2/src`, package V2 IDL/types, or target V2 IDL/types. |

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
- Soft borrow/liquidation, LLAMMA-style liquidation, Jupiter or external
  aggregator conversion routing, explicit hedge premium pricing,
  user-selectable settlement side, and stale locked collateral-factor machinery
  remain intentionally out of scope until separate reviewed specs are merged.
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
