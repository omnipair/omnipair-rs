# Omnipair V2 Owner Signoff Checklist

Use this checklist with `RELEASE_CHECKLIST.md` before declaring the standalone
V2 market program production-ready. The local program gates can be completed by
engineering; the signoffs below require the relevant owners to review the final
branch, deployed artifacts, or target-cluster behavior.

## Signoff Register

| Area | Owner | Status | Evidence |
| --- | --- | --- | --- |
| Security review | TBD | Pending | Fresh review report or approval link. |
| App/front-end routing | TBD | Pending | App PR, staging URL, or routing test notes. |
| SDK/package interface | TBD | Pending | Package diff, typed usage test, or release approval. |
| Indexing/events | TBD | Pending | Indexer config PR or decoded event sample. |
| Analytics/reporting | TBD | Pending | Metric mapping or dashboard validation notes. |
| Aggregator/router integration | TBD | Pending | Quote/swap adapter notes or integration test. |
| Deployment/Squads | TBD | Pending | Buffer, proposal, approval, and execution links. |
| Post-deploy smoke tests | TBD | Pending | Target-cluster smoke test transaction signatures. |

Allowed status values: `Pending`, `Approved`, `Blocked`, `N/A`.

## Security Review

- Confirm the reviewed source is the final standalone `programs/omnipair-v2`
  tree and not an older mixed V1/V2 implementation.
- Review the core invariants listed in `programs/omnipair-v2/README.md`.
- Review the cached-spot EMA flow and pre-action risk snapshots for swap and
  liquidity-add paths.
- Review liquidity-EMA daily limits and spot/K circuit breakers.
- Review floating yLP liquidity, matched yLP redemption, and Token-2022
  transfer checkpointing.
- Review fee liabilities and settlement paths for yLP, hLP, operator,
  protocol, and unallocated buckets.
- Review fixed debt, recognized collateral, normalized valuation, and
  liquidation/insurance/socialization accounting.
- Review Token-2022 constraints and measured inventory-credit settlement.
- Confirm soft borrow and soft liquidation remain disabled unless a separate
  reviewed spec has been merged.
- Confirm LLAMMA-style liquidation, Jupiter/external aggregator conversion
  routing, explicit hedge premium pricing, user-selectable settlement side, and
  stale locked collateral-factor machinery remain out of scope unless separate
  reviewed specs have been merged.

## App / Front-End

- Route new V2 market creation, liquidity, swap, lending, liquidation,
  insurance, yield, protocol-fee, and hedge flows to `OMNIPAIR_V2_PROGRAM_ID`.
- Keep legacy V1 routes available for existing pair positions.
- Do not sort V2 market mints client-side; creator-chosen base/quote order
  defines the market and displayed price direction.
- Display yLP as floating reserve-side yield LP shares.
- Display hLP as aggregate hedged LP vault shares with underlying borrowed
  debt, not as wrapped yLP.
- Surface reduce-only behavior and emergency reduce-only expectations.

## SDK / Package Interface

- Use `IDL_V2`, `OmnipairV2`, and `OMNIPAIR_V2_PROGRAM_ID` for V2 flows.
- Use V2 PDA helpers from `packages/program-interface/src/constants.ts`.
- Confirm V2 IDL and generated TypeScript copies match `target/idl` and
  `target/types` artifacts from the release build.
- Confirm V1 helpers and types remain available for legacy pair flows.
- Confirm consumer examples do not reuse V1 `Pair` decoders for V2 `Market`
  accounts.

## Indexing And Analytics

- Subscribe to the standalone V2 program ID and V2 IDL events.
- Use `MarketEventMetadata.market` as the V2 market key.
- Store V1 pair metrics and V2 market metrics separately at the source level.
- Track yLP supply, hLP vault-owned yLP, hLP supply, hLP debt, recognized
  collateral, insurance, fee liabilities, and market health as separate V2
  metrics.
- Decode `LiquidityAdded`, `LiquidityRemoved`, `SwapExecuted`,
  `MarketDebtUpdated`, `PositionLiquidated`, yield, protocol-fee, hedge, and
  insurance events from the V2 IDL.
- Confirm analytics labels use V2 market terminology and do not present V2 as a
  renamed V1 pair.

## Aggregators And Routers

- Treat V2 `swap` as a distinct venue/source from V1 `swap`.
- Always pass `min_asset_out` and quote with the V2 reserve floor in mind.
- Do not assume V2 yLP behaves like a fixed-principal protected LP token.
- Respect reduce-only mode and risk/circuit-breaker failures.
- Confirm Token-2022 transfer-fee assets are quoted against measured inventory
  behavior where relevant.

## Deployment And Verification

- Confirm `programs/omnipair-v2/src/lib.rs` declares the intended program ID.
- Build the verifiable binary with production features and embedded
  `GIT_REV`/`GIT_RELEASE` metadata.
- Deploy the upgrade buffer through the documented workflow with `program=v2`.
- Transfer upgrade buffer authority to the configured Squads vault.
- Create, approve, and execute the Squads upgrade proposal.
- Verify the deployed binary with `solana-verify` using trailing cargo args for
  `--features production` and the release metadata config.
- Submit the verified build to the OtterSec registry.

## Post-Deploy Smoke

Record target-cluster transaction signatures for:

- market initialization;
- add liquidity and remove liquidity;
- claim yLP and hLP yield;
- swap with slippage protection;
- deposit collateral, borrow, repay, and withdraw idle collateral;
- healthy liquidation rejection;
- unhealthy liquidation on a controlled test market;
- insurance-backed liquidation path;
- open hedge and close hedge;
- reduce-only mode rejection for risk-increasing paths.
