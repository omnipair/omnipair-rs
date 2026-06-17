# Omnipair V2

Omnipair V2 is a separate market architecture program. V1 remains the legacy GAMM pair program in `programs/omnipair`; V2 owns its own market accounts, claim tokens, hedge tokens, risk books, instructions, events, and IDL surface.

## Source Boundaries

- `instructions/`: Anchor account validation, token movement, slippage checks, and event emission.
- `transitions/`: atomic accounting mutations that return receipts for events and tests.
- `state/`: account layouts, local invariants, and market/state methods.
- `tokens/`: protocol meaning and validation for externally transferable claim-token (`omLP`) and hedge-token (`h-omLP`) mints.
- `math/`: fixed-point, AMM, EMA, valuation, and circuit-breaker helpers.
- `utils/`: remaining shared accounting helpers used across transitions.

Instruction modules are split by market domain: `market`, `reserve`, `staking`,
`spot`, `lending`, `liquidation`, and `hedge`.

Instruction names are clean in the V2 namespace: `initialize`, `update_config`, `set_reduce_only`, `swap`, `add_liquidity`, `remove_liquidity`, `stake`, `unstake`, `claim_fees`, `claim_market_fees`, `open_hedge`, `claim_hedge_fees`, `close_hedge`, `deposit_collateral`, `withdraw_collateral`, `borrow`, `repay`, `deposit_insurance`, and `liquidate`.

`set_reduce_only` may be signed by the market operator or by the configured
emergency reduce-only authority.

## Integration Surface

V2 is integrated as its own program, not as a versioned instruction set inside
the legacy V1 program.

- Program crate: `programs/omnipair-v2`
- Program name: `omnipair_v2`
- Mainnet/devnet/localnet ID: `358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv`
- IDL: `target/idl/omnipair_v2.json`
- TypeScript bindings: `packages/program-interface/src/types_v2.ts`
- SDK PDA helpers: `packages/program-interface/src/constants.ts`

The SDK exports both generations. Use `OMNIPAIR_PROGRAM_ID` for V1 pair flows
and `OMNIPAIR_V2_PROGRAM_ID` for V2 market flows.

## Integrator Handoff

Treat V2 as a new venue/source under the Omnipair brand. It is not a V1 pair
account with renamed fields.

SDK consumers:

- instantiate V2 with `IDL_V2`, `OmnipairV2`, and
  `OMNIPAIR_V2_PROGRAM_ID`;
- derive markets and vaults through the V2 PDA helpers in
  `packages/program-interface/src/constants.ts`;
- keep V1 calls on `IDL`, `OMNIPAIR_PROGRAM_ID`, and V1 pair PDA helpers.

Apps:

- route new market creation, liquidity, swap, borrow, repay, liquidation, fee,
  and hedge flows to the V2 program ID;
- keep legacy V1 routes available for existing pair positions;
- never sort V2 market mints client-side, because creator-chosen base/quote
  order defines the market and price direction.

Indexers:

- subscribe to the standalone V2 program ID and V2 IDL events;
- use `MarketEventMetadata.market` as the primary market key for V2 events;
- read `Market`, `StakePosition`, `MarginPosition`, and `HedgePosition`
  accounts from the V2 IDL rather than V1 pair decoders;
- keep claim-token principal, hedge-token supply, debt, insurance, fee
  liabilities, and buffer shares as separate V2 metrics.

Aggregators and routers:

- treat V2 `swap` as a separate source from V1 `swap`;
- quote with the V2 market reserve floor in mind and always pass
  `min_asset_out`;
- do not assume claim tokens represent an LP exchange rate. Claim tokens are
  fixed-principal assets; fee rights require staking matched claim tokens and
  buffer shares.

## PDA Map

The public SDK helper names are the preferred integration entry points:

| Account | Seeds | SDK helper |
| --- | --- | --- |
| `Market` | `market_v2`, `base_mint`, `quote_mint`, `params_hash` | `deriveMarketAddress` / `deriveMarketV2Address` |
| Reserve vault | `market_reserve`, `market`, `asset_mint` | `deriveMarketReserveVaultAddress` |
| Collateral vault | `market_collateral`, `market`, `asset_mint` | `deriveMarketCollateralVaultAddress` |
| Fee vault | `market_fee`, `market`, `asset_mint` | `deriveMarketFeeVaultAddress` |
| Stake vault | `market_stake`, `market`, `claim_token_mint` | `deriveMarketStakeVaultAddress` |
| Stake position | `stake`, `market`, `owner`, `asset_mint` | `deriveStakePositionAddress` |
| Margin position | `margin`, `market`, `owner` | `deriveMarginPositionAddress` |
| Hedge vault | `hedged`, `market`, `claim_token_mint` | `deriveHedgeVaultAddress` |
| Hedge position | `hedge_position`, `market`, `owner`, `asset_mint` | `deriveHedgePositionAddress` |
| Insurance reserve vault | `insurance`, `market`, `asset_mint` | `deriveInsuranceReserveAddress` |

Market creators choose `base_mint` and `quote_mint`; V2 does not sort or
canonicalize mint order. Price displays should read as quote per base.

## Integration Flows

### Market Creation

`initialize` creates a fresh isolated market. The caller supplies base/quote
asset mints, base/quote claim-token mints, base/quote hedge-token mints, and all
market vault PDAs. Claim and hedge mints must be fee-free mints controlled by
the market authority and use the same decimals as their asset side.

### Liquidity

`add_liquidity` transfers asset inventory into a side's reserve vault. The
deposit is split into:

- transferable claim tokens minted to the LP;
- non-transferable junior buffer shares credited on the LP's `StakePosition`.

`remove_liquidity` burns claim tokens and returns fixed 1:1 principal only.
It does not return fees, does not rebase, and does not release buffer shares.
Buffer shares remain on the stake position as junior risk-capital accounting
that can be matched with claim tokens for fee eligibility.

`stake` moves claim tokens into the market stake vault and matches them with
buffer shares. `unstake` returns claim tokens and moves the paired buffer shares
back to `available_buffer_share_amount` on the stake position.

### Fees

Swap fees are held in fee vaults and recorded as liabilities:

- staker fees use `fee_growth_index_nad` and are claimed with `claim_fees`;
- hedge-wrapper fees use `hedged_fee_growth_index_nad` and are claimed with
  `claim_hedge_fees`;
- operator/protocol buckets are claimed with `claim_market_fees`.

Unallocated LP fees are carried forward into the next active stake index.
Claim-token principal never compounds fee income.

### Spot Swaps

`swap` transfers `asset_in` into the reserve vault, moves the configured fee to
the fee vault, pays `asset_out` from the opposite reserve vault, then enforces
the post-swap reserve floor. Integrators should provide slippage protection via
`min_asset_out`.

### Lending

`deposit_collateral` and `withdraw_collateral` manage idle margin inventory.
`borrow` recognizes only debt-bearing collateral on the opposite side and
transfers the borrowed asset from the borrowed side's reserve while recording
fixed debt shares. Idle same-side collateral does not improve market health.
`repay` reduces fixed debt shares and releases recognized collateral
proportionally.

`soft_borrow_enabled` is currently rejected by config validation. Soft
liquidation is intentionally not live.

### Liquidation And Insurance

`deposit_insurance` funds the junior insurance reserve for a side. `liquidate`
repays insolvent debt, seizes borrower collateral, draws insurance if needed,
and only then socializes remaining bad debt to LP reserves subject to the
liquidator's `max_socialized_loss` bound.

### Hedged Claim Wrappers

`open_hedge` escrows the selected side's claim tokens one-to-one and mints hedge tokens.
`close_hedge` burns hedge tokens and returns the escrowed claim tokens. Hedge
tokens can receive routed hedge fees, but they do not grant staking rights and
do not include buffer shares.

## Event Surface

Indexers should consume V2 events from the standalone V2 IDL:

| Event | Emitted by |
| --- | --- |
| `MarketCreated` | `initialize` |
| `MarketUpdated` | `update_config`, `set_reduce_only` |
| `MarketHealthUpdated` | config, swap, remove_liquidity, withdraw_collateral, borrow, repay, fee claims, hedge open/close, liquidation health refreshes |
| `LiquidityAdded` | `add_liquidity` |
| `LiquidityRemoved` | `remove_liquidity` |
| `MarketStakeUpdated` | `stake`, `unstake` |
| `MarketFeesClaimed` | `claim_fees` |
| `MarketFeeLiabilityClaimed` | `claim_market_fees` |
| `SwapExecuted` | `swap` |
| `MarketCollateralDeposited` | `deposit_collateral` |
| `MarketCollateralWithdrawn` | `withdraw_collateral` |
| `MarketDebtUpdated` | `borrow`, `repay` |
| `MarketInsuranceFunded` | `deposit_insurance` |
| `PositionLiquidated` | `liquidate` |
| `MarketHedgeOpened` | `open_hedge` |
| `MarketHedgeClosed` | `close_hedge` |
| `MarketHedgeFeesClaimed` | `claim_hedge_fees` |

Every V2 event carries `MarketEventMetadata` with the signer, market, and slot.

## Core Invariants

- Claim tokens are fixed-principal `omLP` assets. They do not rebase, compound fees, or use a dynamic exchange rate.
- Reserve floors must cover protected claim token supply plus required buffer on each market side.
- Deposits split into protected claim amount and buffer share amount; only the claim amount is minted as transferable claim tokens.
- Fee rights require matched staked claim tokens plus buffer shares. Unstaked claim tokens remain principal-only redemption claims.
- Fees are non-compounding liabilities. They are routed through fee ledgers, fee growth indexes, and explicit claim paths.
- Buffer-ratio changes are locked while active stake or staker LP fee liabilities exist, including carried-forward no-stake fees.
- Config updates must preserve the market-health floor for existing effective debt after risk and health are refreshed.
- Market health uses recognized debt-bearing collateral only. Idle collateral contributes zero to borrow health.
- Fixed debt is valued in normalized debt units. Hedged overlay debt is gamma-weighted against liquidity EMA, while fixed and soft debt remain fully effective.
- Hedge opens must preserve the market-health floor after gamma-weighted overlay debt is refreshed.
- Risk books roll EMA values from cached spot and liquidity observations, then store the current observation for the next refresh.
- Liquidity add/remove, swap, borrow, repay, liquidation, fee claim, and hedge close paths check risk circuit breakers where they increase or settle risk against market prices.
- Liquidation reduces only insolvent debt and follows the waterfall: borrower collateral, liquidator repayment and incentive, insurance reserve, then LP socialization.
- Hedge tokens are `h-omLP` wrappers that escrow claim tokens one-to-one and unwrap back into claim tokens. They do not create staking rights.
- Inventory-native settlement is used for reserves, collateral, fees, insurance, claims, and hedge vaults.

## Production Caveats

Before V2 should be treated as production-ready:

- run a fresh end-to-end security review against the final standalone V2 program;
- finish the V2 deployment and release checklist in
  `programs/omnipair-v2/RELEASE_CHECKLIST.md`;
- keep soft borrow and soft liquidation disabled until a separate spec is ready.

## Verification

Useful focused checks while changing V2:

```bash
cargo test -p omnipair-v2 --lib -- --nocapture
cargo check -p omnipair-v2
anchor build -p omnipair-v2
npm run build --prefix packages/program-interface
yarn test-litesvm
```

Run package interface builds when public IDL, account, event, seed, or instruction shapes change.

The current V2 review gates are:

- `cargo fmt -p omnipair-v2 -- --check`
- `cargo check -p omnipair-v2 --lib`
- `cargo test -p omnipair-v2 --lib -- --nocapture`
- `anchor build -p omnipair-v2`
- `npm run build --prefix packages/program-interface`
- `yarn test-litesvm`

`yarn test-litesvm` reports V2 instruction coverage separately from legacy V1 coverage. V2 is expected to cover all standalone V2 instructions in that report.

## Legacy V1 Baseline

V1 remains the legacy program and is not expected to become clean as part of V2 review. As of the current branch baseline, `cargo test -p omnipair --lib` has 5 known failures:

- `v1::state::rate_model::tests::test_default_matches_original_low_util`
- `v1::state::rate_model::tests::test_default_matches_original_high_util`
- `v1::state::rate_model::tests::test_faster_half_life_adjusts_quicker`
- `v1::state::rate_model::tests::test_uncapped_rate_grows_exponentially`
- `shared::gamm_math::tests::manipulation_bounded_by_ema`

Treat new V1 failures beyond that list as regressions, and keep V2 changes out of the legacy V1 instruction surface unless the change is explicitly scoped as V1 work.
