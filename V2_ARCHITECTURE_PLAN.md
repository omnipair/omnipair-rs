# Omnipair v2 Architecture Plan

## Current Status

This plan has moved from proposal to implementation. The current branch has a
standalone `programs/omnipair-v2` Anchor program while the legacy
`programs/omnipair` program remains the V1 surface.

Implemented V2 product surface:

- `initialize`, `update_config`, and `set_reduce_only`
- `add_liquidity` and `remove_liquidity`
- `stake`, `unstake`, `claim_fees`, `claim_market_fees`, and `claim_hedge_fees`
- `swap`
- `deposit_collateral`, `withdraw_collateral`, `borrow`, `repay`, and `liquidate`
- `deposit_insurance`
- `open_hedge` and `close_hedge`

Implemented V2 architecture pieces:

- standalone V2 program ID and IDL;
- V1-style one-instruction-per-file layout;
- market-based state, vaults, seeds, events, and SDK helpers;
- claim-minus-buffer liquidity accounting;
- buffer shares as non-transferable junior risk-capital accounting units that
  stay with the stake position after protected claim-token principal is
  redeemed;
- fixed 1:1 claim-token principal;
- staking-gated non-compounding fee indexes;
- market reserve floors on swaps and withdrawals;
- fixed-token debt with recognized-collateral market health;
- cached spot observations for EMA updates;
- pre-action risk snapshots for swaps and liquidity adds, so EMA bootstraps from
  the previous observed market state rather than same-instruction post-state
  spot;
- liquidity-EMA daily limits and circuit breakers;
- buffer-ratio updates locked while active stake, allocated staker fee
  liabilities, or carried-forward no-stake LP fee liabilities exist;
- liquidation with collateral seizure, insurance draw, and LP socialization;
- h-claim hedge wrappers as 1:1 claim-token overlays;
- hedge opens bounded by post-open market health after gamma-weighted overlay
  debt refresh;
- config updates are conservative: after a config change, refreshed market
  health must still satisfy the configured health floor for existing debt.

Remaining work before treating V2 as production-ready:

- run a fresh end-to-end security review against the final standalone program;
- finish deployment/release checklist review for mainnet and SDK consumers;
- keep soft borrow / soft liquidation disabled until a separate spec is ready.

## Purpose

This document captures the current design direction for Omnipair v2 so a coding model can work from one coherent brief.

The main conclusion is that v2 should be a separate Solana program rather than live inside the existing v1 program. V2 is not only a feature upgrade. It has a different account model, token model, risk model, event surface, and integration surface. Keeping it inside the v1 program preserves the program ID, but it does not avoid new integration work, and it forces awkward public instruction names like `market_swap`, `market_borrow`, and `market_liquidate`.

A separate program lets v1 remain stable while v2 gets a clean, canonical protocol surface.

## Core Decision

Use two programs:

- `omnipair`: legacy v1 GAMM pair program, kept working as-is.
- `omnipair_v2`: new market architecture program.

Product routing:

- `omnipair.fi`: canonical v2 app.
- `v1.omnipair.fi`: legacy v1 app.
- SDK and analytics should support both and combine metrics under the Omnipair brand.

This should be framed as one protocol brand with two program generations:

- Omnipair v1: legacy GAMM pair program.
- Omnipair v2: market architecture program.

## Why Separate Program

V2 needs new integrations regardless of whether it lives in the old program because integrators still need to understand:

- new market accounts,
- new vault layout,
- new token mints,
- new instructions,
- new events,
- new pricing and liquidity math,
- new liquidation and debt semantics.

The benefit of staying in the same program is mostly program-ID continuity, not automatic compatibility. A separate program gives cleaner code and cleaner external API:

- reclaim simple instruction names like `swap`, `borrow`, `repay`, and `liquidate`;
- avoid mixed v1/v2 IDL catalogs;
- keep events and errors domain-specific;
- simplify audits by making v2 its own state machine;
- avoid keeping v1 baggage in the v2 root module;
- make integrator docs clearer: v1 program is legacy, v2 program is current.

## Non-Goals

- Do not realloc existing v1 accounts.
- Do not perform a forced stateful migration.
- Do not break existing v1 integrations.
- Do not leave temporary migration logic inside v1.
- Do not preserve awkward `market_*` instruction names if v2 has its own program namespace.
- Do not copy MakerDAO's cryptic names directly.

## Migration Strategy

V1 remains live and untouched except for any cleanup needed to remove mixed v2 code from the current branch.

V2 launches as a fresh program with fresh accounts and fresh liquidity. Migration is lazy:

- v1 liquidity stays withdrawable;
- users voluntarily withdraw from v1 and deposit into v2;
- aggregators can integrate v2 as a new venue/source;
- analytics can combine both programs under one Omnipair dashboard;
- the SDK can expose one high-level Omnipair interface while routing to v1 or v2 internally.

## Public Naming Direction

Because v2 gets its own program namespace, public instruction names should be clean and action-oriented.

Preferred public names:

```text
initialize
update_config
set_reduce_only
swap
add_liquidity
remove_liquidity
deposit_collateral
withdraw_collateral
borrow
repay
liquidate
deposit_insurance
stake
unstake
claim_fees
claim_market_fees
open_hedge
claim_hedge_fees
close_hedge
```

Avoid temporal or workaround names:

```text
swap_v2
borrow_v2
market_swap
market_borrow
market_repay
market_liquidate
```

`market` is still a good domain/account concept, but it should not be a forced prefix on every public instruction.

## Source Grouping Direction

Keep the v1 style of grouping related instructions together. That was a good ergonomic choice.

The instruction tree should use user-facing protocol language where it maps cleanly to the product surface, and should split larger domains when that makes review easier. The current implementation keeps `reserve`, `staking`, `hedge`, `lending`, `liquidation`, `spot`, and `market` as separate audit-sized folders.

Current v2 instruction layout:

```text
instructions/
  market/
    initialize.rs
    update_config.rs
    set_reduce_only.rs

  reserve/
    add_liquidity.rs
    remove_liquidity.rs

  staking/
    stake.rs
    unstake.rs
    claim_fees.rs
    claim_market_fees.rs

  hedge/
    open_hedge.rs
    claim_hedge_fees.rs
    close_hedge.rs

  spot/
    swap.rs

  lending/
    deposit_collateral.rs
    withdraw_collateral.rs
    borrow.rs
    repay.rs

  liquidation/
    liquidate.rs
    deposit_insurance.rs
```

Hedging, staking, reserve liquidity, and liquidation now live in separate folders because their account sets and risk checks are different enough that reviewers benefit from narrower modules. The source tree should optimize for how users and auditors follow the protocol state machine.

This gives both:

- v1-style source ergonomics for auditing and development;
- clean public instruction names in the IDL.

Example root dispatch:

```rust
pub fn swap(ctx: Context<Swap>, args: SwapArgs) -> Result<()> {
    spot::Swap::handle(ctx, args)
}

pub fn borrow(ctx: Context<Borrow>, args: BorrowArgs) -> Result<()> {
    lending::Borrow::handle(ctx, args)
}

pub fn liquidate(ctx: Context<Liquidate>, args: LiquidateArgs) -> Result<()> {
    liquidation::Liquidate::handle(ctx, args)
}
```

## Liquidity vs Reserve Naming

Use `liquidity` for user-facing actions.

Use `reserve` for internal custody/accounting and for the current add/remove liquidity instruction folder.

Rationale:

- `liquidity` is what users, LPs, aggregators, dashboards, and integrations understand.
- `reserve` is more precise for internal state like vaults, backing, claim coverage, and accounting floors.

Current split:

```text
instructions/reserve/add_liquidity.rs
instructions/reserve/remove_liquidity.rs
state/ledgers.rs
transitions/reserve.rs
```

Internal names can still use:

```rust
ReserveLedger
reserve_vault
cash_reserve
live_reserve
```

Public instruction names can use:

```rust
add_liquidity
remove_liquidity
```

The current preference is to keep the familiar v1-style `add_liquidity` and `remove_liquidity` names in the new v2 namespace, even if the underlying v2 accounting is one-sided and reserve-backed.

## Token Vocabulary

Solana does not make tokenization visually obvious the way Ethereum does with contracts like `cToken.sol`, `aToken`, `stETH`, or `crvUSD`. In Omnipair v2, names should make it obvious what is an externally transferable SPL token and what is internal accounting.

Naming rules:

- `*Token` means an externally transferable SPL token concept.
- `*Mint` means the SPL mint account.
- `*Vault` means token custody account.
- `*Shares` means internal pro-rata accounting, not necessarily transferable.
- `*Units` means derived accounting weight.
- `*Ledger` means stored accounting balances.
- `*Book` means a broader portfolio, exposure, or risk/accounting model.
- Avoid `token` in internal accounting names unless there is a real SPL mint behind it.

Preferred examples:

```rust
claim_token_mint
hedge_token_mint
claim_token_supply
hedged_claim_token_supply
staked_claim_token_amount
buffer_share_supply
staked_buffer_share_amount
```

Clear accounting names:

```rust
ReserveLedger
ClaimTokenLedger
BufferLedger
FeeLedger
DebtBook
RiskBook
```

Avoid ambiguous names like:

```rust
claim_mint
hedge_mint
staked_claim_supply
buffer_book
```

unless the surrounding module makes the meaning very clear.

## Book vs Ledger

Use `Ledger` when the struct mostly stores balances and accounting totals.

Examples:

```rust
ReserveLedger
ClaimTokenLedger
BufferLedger
FeeLedger
```

Use `Book` when the struct represents a broader financial model, exposure set, or derived risk/accounting view.

Examples:

```rust
DebtBook
RiskBook
```

`BufferBook` is less natural because "buffer book" is not a common finance phrase. `BufferLedger` is clearer.

## Token Modules

Even though Solana tokens are mints/accounts rather than contracts, v2 should give tokenized assets first-class source modules.

Suggested layout:

```text
tokens/
  claim_token.rs
  hedge_token.rs
```

These files should define protocol meaning, mint constraints, supply accounting, vault relationships, and mint/burn/escrow helpers. They are not separate SPL token programs. They are documentation and implementation boundaries for tokenized protocol assets.

## Atomic State Transition Philosophy

Adopt the useful part of MakerDAO's style: small accounting kernels, precise state transitions, and invariant-centered mutation.

Do not copy MakerDAO's intentionally terse or cryptic naming. The goal is a readable Omnipair accounting language.

Instructions should be thin adapters around atomic state transitions.

Pattern:

```text
1. Validate accounts.
2. Measure real token movement if needed.
3. Construct a transition.
4. Apply the transition to protocol books.
5. Assert invariants.
6. Settle token transfers.
7. Emit a receipt/event.
```

Avoid scattered direct mutations outside transition code:

```rust
market.side0.reserve_ledger.cash_reserve = ...
market.debt_book.fixed_debt0_shares = ...
position.recognized_collateral1_for_debt0 = ...
```

Prefer named transitions:

```rust
Borrow::apply(...)
Repay::apply(...)
Swap::apply(...)
Liquidation::apply(...)
Stake::apply(...)
Redeem::apply(...)
```

Each transition should return a receipt used by events and tests.

Example shape:

```rust
pub struct Borrow {
    pub borrow_asset: MarketAsset,
    pub borrow_amount: u64,
    pub min_health_bps: u64,
}

pub struct BorrowReceipt {
    pub owner: Pubkey,
    pub debt_asset_mint: Pubkey,
    pub debt_delta: i64,
    pub fixed_debt0: u128,
    pub fixed_debt1: u128,
    pub health0_bps: u64,
    pub health1_bps: u64,
}

impl Borrow {
    pub fn apply(
        self,
        market: &mut Market,
        position: &mut MarginPosition,
    ) -> Result<BorrowReceipt> {
        // single named mutation path
    }
}
```

## Suggested Program Layout

If v2 is split into a separate program, aim for this shape:

```text
programs/omnipair-v2/src/
  lib.rs
  constants.rs
  errors.rs
  events.rs

  state/
    market.rs
    side.rs
    positions.rs
    ledgers.rs
    risk.rs

  instructions/
    market/
      initialize.rs
      update_config.rs
      set_reduce_only.rs
    reserve/
      add_liquidity.rs
      remove_liquidity.rs
    staking/
      stake.rs
      unstake.rs
      claim_fees.rs
      claim_market_fees.rs
    hedge/
      open_hedge.rs
      claim_hedge_fees.rs
      close_hedge.rs
    spot/
      swap.rs
    lending/
      deposit_collateral.rs
      withdraw_collateral.rs
      borrow.rs
      repay.rs
    liquidation/
      liquidate.rs
      deposit_insurance.rs

  transitions/
    reserve.rs
    staking.rs
    fee.rs
    debt.rs
    collateral.rs
    swap.rs
    liquidation.rs
    hedge.rs
    insurance.rs

  tokens/
    claim_token.rs
    hedge_token.rs

  math/
    gamm.rs
    fixed_point.rs
    risk.rs

  shared/
    account.rs
    token.rs
```

The exact file split can evolve, but avoid one huge `market.rs` containing all state, risk math, token semantics, accounting transitions, helpers, seed macros, and tests.

## State and Transition Boundaries

State structs should mostly describe stored data and small local invariants.

Good examples:

```rust
ReserveLedger
ClaimTokenLedger
BufferLedger
FeeLedger
DebtBook
RiskBook
MarketSide
MarginPosition
StakePosition
HedgePosition
```

Transition modules should coordinate coupled mutations across those structs.

Unlike `instructions/`, the global `transitions/` folder can use more technical accounting terminology. This is where names like `reserve`, `staking`, `fee`, `debt`, `collateral`, `swap`, `liquidation`, `hedge`, and `insurance` are useful. The public instruction layer should stay simple; the transition layer should be precise.

Examples:

```text
transitions/debt.rs
transitions/collateral.rs
transitions/liquidation.rs
transitions/reserve.rs
transitions/staking.rs
transitions/fee.rs
transitions/swap.rs
```

Instruction modules should:

- validate canonical accounts;
- perform token transfers or measure token credits;
- call one transition;
- emit events from the transition receipt.

## Side Selection

Avoid long-term reliance on booleans like:

```rust
debt_asset_is_asset0: bool
asset_in_is_asset0: bool
```

Prefer a typed side selector:

```rust
pub enum MarketAsset {
    Base,
    Quote,
}
```

This can expose helpers like:

```rust
market.side(asset)?;
market.side_mut(asset)?;
asset.opposite()
```

Booleans are acceptable during migration, but the target style should make side semantics explicit.

## Event and Error Surface

In the separate v2 program, keep events and errors cleanly v2-specific.

Avoid mixed catalogs containing both v1 and v2 events.

Events should be derived from transition receipts where possible:

```rust
let receipt = Borrow::new(args).apply(&mut market, &mut position)?;
emit_cpi!(DebtUpdated::from(receipt));
```

This keeps events aligned with actual state changes.

## Tests

Use both direct unit tests and LiteSVM integration tests.

Unit tests:

- transition-level accounting;
- ledger invariants;
- risk math;
- fee allocation;
- liquidation edge cases;
- token supply accounting.

LiteSVM tests:

- account constraints;
- real SPL token transfers;
- mint/burn behavior;
- PDA/vault correctness;
- full user flows.

Instruction coverage tracking can remain a useful checklist, but do not treat it as true behavioral coverage. A test that only checks an IDL or PDA should not imply the instruction behavior is covered.

## SDK and Integration Strategy

The SDK can expose clean high-level names and route internally:

```ts
omnipair.swap(...)
omnipair.borrow(...)
omnipair.repay(...)
omnipair.liquidate(...)
```

The SDK may support both v1 and v2 under one package, but it should make the account model explicit.

Analytics should combine both programs under the Omnipair brand while still tracking v1 and v2 separately at the source level.

Frontend routing:

```text
omnipair.fi       -> v2 canonical app
v1.omnipair.fi    -> legacy v1 app
```

## Implementation Status By Phase

### Phase 1: Split Program Architecture

Status: implemented.

- Created the standalone `programs/omnipair-v2` program crate.
- Moved V2 code into the standalone program.
- Restored the existing `omnipair` program to a clean V1-only surface.
- Kept V1 public instruction names and integrations stable.

### Phase 2: Reclaim Public Names

Status: implemented.

- Public V2 instructions use clean names such as `swap`, `borrow`, `repay`,
  and `liquidate`.
- Domain clarity lives in account, state, event, and folder names instead of
  workaround instruction prefixes.

### Phase 3: Preserve Grouped Instruction Layout

Status: implemented.

- V2 keeps V1-style domain grouping.
- Instructions are split across `instructions/reserve`, `instructions/staking`,
  `instructions/hedge`, `instructions/lending`, `instructions/liquidation`,
  `instructions/spot`, and `instructions/market`.
- Staking, market fee claiming, hedge fee claiming, and liquidation have
  narrow review surfaces.

### Phase 4: Clean Token Vocabulary

Status: implemented, with one open product-language decision.

- Claim and hedge token concepts are explicit.
- Ambiguous mint, supply, and share fields were renamed where needed.
- `tokens/claim_token.rs` and `tokens/hedge_token.rs` define token constraints.
- SPL token concepts are separated from internal accounting units.
- Open decision: whether retained junior buffer shares need a public product
  term beyond the current internal accounting language.

### Phase 5: Introduce Atomic Transitions

Status: implemented.

- Coupled state mutation logic lives under `transitions/`.
- Receipt structs drive event emission and tests.
- Instruction handlers are thin account/token adapters around transition calls.
- Transition boundaries assert the relevant local invariants.

### Phase 6: Improve Modularity

Status: implemented.

- State is split into market, side, ledgers, positions, config, health, and risk
  modules.
- Math helpers live under `math/` and `utils/`.
- Ledgers remain small and invariant-focused.
- Risk math has dedicated modules for risk-book and fixed-point behavior.

### Phase 7: Rebuild Tests

Status: implemented for local review coverage.

- Transition, state, math, and helper modules have direct unit/property tests.
- LiteSVM covers real program behavior and all 19 standalone V2 instructions.
- Coverage tracking reports V2 instruction smoke coverage separately from the
  legacy V1 surface.

### Phase 8: Integration and Product Surface

Status: partially implemented.

- Separate V2 IDL and TypeScript bindings exist.
- SDK constants expose V2 program ID and PDA helpers.
- V1 documentation and the V1 program remain available for legacy users.
- Remaining production work: finish app/front-end routing, aggregator notes,
  analytics/indexer handoff, deployment review, and external security signoff.

## Success Criteria

- V1 remains stable and usable.
- V2 has a clean IDL with no awkward version-prefixed or `market_*` workaround instruction names.
- Source code keeps v1-style domain grouping for related instructions.
- Tokenized assets are obvious from names.
- Internal accounting units are clearly separated from SPL tokens.
- Core state transitions are atomic, named, tested, and auditable.
- No forced migration or realloc path is required.
- Integrators can reason about v2 as a clean new program while analytics and product surfaces still present one Omnipair brand.
