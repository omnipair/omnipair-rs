# V2 Naming Polish Plan

## Goal

Make the v2 code read with a clean user-facing vocabulary at the instruction boundary, while preserving more technical accounting vocabulary inside state, transitions, math, and ledgers.

This should be a naming-only cleanup. No account layouts, instruction discriminators, math behavior, or state transition semantics should change unless explicitly called out.

## Current Status

The main naming pass is implemented in the standalone `omnipair-v2` program:

- public instruction names are action-oriented (`swap`, `borrow`, `repay`,
  `liquidate`, `add_liquidity`, `remove_liquidity`);
- instruction files use V1-style domain folders and one file per instruction;
- market identity uses `base` / `quote` terminology;
- V2 program code no longer exposes `token0`, `token1`, `asset0`, or `asset1`
  terminology;
- events have product-facing names such as `LiquidityAdded`,
  `LiquidityRemoved`, `SwapExecuted`, and `PositionLiquidated`;
- `MarketEventMetadata::new` returns `Result` and does not use production
  `unwrap()`;
- creator-chosen base/quote order is covered by tests.

Remaining naming/product decision:

- decide whether retained junior buffer shares need a public product term
  beyond the current internal accounting language.

Ongoing hygiene:

- preserve additional useful V1 explanatory comments only where copied math or
  accounting still shares the same assumptions.

## Principle

Public instruction code should use product language:

- `liquidity`
- `lending`
- `spot`
- `market`
- `add_liquidity`
- `remove_liquidity`
- `borrow`
- `repay`
- `swap`

Market identity should use market language:

- `base`
- `quote`
- `base_mint`
- `quote_mint`
- `base_vault`
- `quote_vault`
- `base_amount`
- `quote_amount`

Avoid `token0`, `token1`, `asset0`, and `asset1` in public-facing v2 code unless the code is explicitly dealing with a low-level index.

Internal transition code can use accounting language:

- `reserve`
- `debt`
- `collateral`
- `buffer`
- `claim_token`
- `hedge_token`
- `ledger`
- `shares`
- `units`

The boundary should feel intentional: users and integrators see the simple names; protocol internals keep the precise names.

For a market like `SOL/USDC`, `SOL` is the base asset and `USDC` is the quote asset. Price should read as quote per base.

Pure math and invariant code can use coordinate language:

- `x`
- `y`
- `dx`
- `dy`
- `xp` for precision-adjusted balances, if needed

This follows the Curve-style convention where `x` and `y` are curve coordinates, `dx` is the amount added to one side, and `dy` is the amount removed from the other side.

## Scope

### 1. Rename instruction account structs and args

Keep public ixn names and files as they are, but align the Anchor context and args names with the ixn vocabulary.

Suggested changes:

- `DepositReserveArgs` -> `AddLiquidityArgs`
- `DepositReserve` -> `AddLiquidity`
- `RedeemClaimArgs` -> `RemoveLiquidityArgs`
- `RedeemClaim` -> `RemoveLiquidity`

Check for similar leaks in other instruction files. If a struct is part of `instructions/liquidity`, it should usually say `Liquidity` unless it is explicitly an internal helper.

### 2. Keep reserve naming in transitions

Do not rename `transitions/reserve.rs` or the reserve transition types unless there is a clear reason.

Good internal names:

- `AddLiquidity`
- `RemoveLiquidity`
- `ReserveReceipt`
- `ReserveDelta`
- `ReserveBalance`

If the transition already represents the protocol accounting operation, technical names are acceptable there.

### 3. Rename market sides to base and quote

Replace public and semi-public `token0` / `token1` / `asset0` / `asset1` terminology with `base` / `quote`.

Good target names:

- `base_mint`
- `quote_mint`
- `base_vault`
- `quote_vault`
- `base_amount`
- `quote_amount`
- `base_reserve`
- `quote_reserve`
- `base_debt`
- `quote_debt`

For directional operations like swaps, keep call-local names like:

- `asset_in`
- `asset_out`
- `amount_in`
- `amount_out`

Those names are clearer than forcing every per-call value into base/quote terminology.

### 4. Use x/y/dx/dy inside pure math

Use Curve-style coordinate names inside pure invariant math and small math helpers.

Good places for `x`, `y`, `dx`, and `dy`:

- `math/gamm.rs`
- `math/fixed_point.rs`
- local helpers that only compute curve outputs
- tests that are specifically asserting curve math

Avoid leaking `x` and `y` into:

- instruction account structs
- public args
- state fields
- events
- vault movement code
- debt, collateral, or fee accounting

The intended naming boundary is:

- `base` / `quote` describe the market.
- `asset_in` / `asset_out` describe a transaction.
- `x` / `y` / `dx` / `dy` describe curve math.

If a transition is mostly orchestrating state movement, prefer semantic names. If a function is pure invariant calculation, prefer coordinate names.

### 5. Let market creators choose base and quote

Remove the canonical ordering check from market initialization.

The initializer should not sort mints or reject a pair because the chosen `base_mint` / `quote_mint` order is not canonical. The creator's chosen order defines the market's price direction.

Still keep basic validation:

- `base_mint` and `quote_mint` must be different.
- all vaults and ledgers must match the chosen base/quote direction.
- tests should cover initializing the same pair in the non-canonical direction.

If reversed markets should be disallowed later, handle that with an explicit registry policy. Do not use a hidden token-ordering rule that changes the meaning of base and quote.

### 6. Restore useful v1 comments in copied code

For code copied from v1 into v2, bring back any useful explanatory comments that were removed during the move.

Guidelines:

- restore comments that explain protocol invariants, rounding behavior, risk assumptions, or non-obvious accounting.
- adapt comments if the v2 names changed, especially `token0` / `token1` to `base` / `quote`.
- skip stale comments that describe old v1-only instruction structure.
- do not add noise comments for obvious assignments or one-line wrappers.

This is mainly about preserving hard-won context from v1 while making the v2 naming cleaner.

### 7. Review event names

Decide whether event names should be product-facing or accounting-facing.

Candidate event polish:

- `MarketReserveDeposited` -> `LiquidityAdded`
- `MarketClaimRedeemed` -> `LiquidityRemoved`
- `MarketSwapEvent` -> `SwapExecuted`
- `MarketLiquidated` -> `PositionLiquidated`

Only rename events if downstream consumers are ready for the change. If analytics compatibility matters more, leave event names as-is for now and document the reason.

### 8. Remove production unwraps where easy

Review event metadata helpers and similar code for avoidable `unwrap()` usage.

Example target:

- `MarketEventMetadata::new`

Preferred direction:

- return `Result<MarketEventMetadata>`
- or pass the slot in from the instruction handler

This is secondary to naming. Do it only if the diff stays small.

### 9. Defer typed side selectors

There are several selectors like:

- `asset_in_is_asset0`
- `borrow_asset_is_asset0`
- `repay_asset_is_asset0`
- `debt_asset_is_asset0`
- `market_side_index`

These can stay for now because they are IDL-simple and low-risk.

Later, consider typed wrappers or enums such as:

- `AssetSide`
- `DebtSide`
- `MarketSideIndex`

Do not include this in the first naming polish pass unless it is extremely localized.

## Non-Goals

- No state migration.
- No account realloc.
- No math rewrites.
- No instruction behavior changes except the explicit market initialization base/quote ordering change.
- No large `state/market.rs` split.
- No shared crate extraction.
- No v1 changes.

## Suggested Order

1. Rename the liquidity instruction account structs and args.
2. Rename market-side terminology from token/index language to base/quote language.
3. Use `x` / `y` / `dx` / `dy` in pure curve math where it improves clarity.
4. Remove the canonical ordering check from market initialization and add coverage for creator-chosen base/quote order.
5. Restore accurate v1 comments in copied v2 code.
6. Update imports, handlers, tests, and IDL-facing references.
7. Run `cargo check -p omnipair-v2 --lib`.
8. Run `cargo test -p omnipair-v2 --lib`.
9. Review event naming separately and decide whether to rename now or leave as an analytics-compatible follow-up.

## Success Criteria

- Public instruction files read in user-facing terms.
- Market identity reads as base/quote, not token0/token1.
- Pure math uses coordinate names where that makes formulas clearer.
- Market initialization respects creator-chosen base/quote order.
- Internal transition files still read in accounting terms.
- Useful v1 comments are preserved where copied code still relies on the same reasoning.
- `cargo check -p omnipair-v2 --lib` passes.
- `cargo test -p omnipair-v2 --lib` passes.
- No v1 program behavior or namespace changes.
