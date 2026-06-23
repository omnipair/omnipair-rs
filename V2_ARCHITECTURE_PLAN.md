# Omnipair V2 Final yLP / hLP Architecture

This document captures the current V2 direction. It supersedes the earlier
protected-principal LP, public staking, and underwriter-market plans.

## Summary

V2 is a standalone `omnipair_v2` program with a market-native account model.
V1 remains unchanged for legacy pairs.

The final V2 product surface is:

- `yLP_X` and `yLP_Y`: floating reserve-side yield LP shares.
- `hLP_X` and `hLP_Y`: aggregate 2x leveraged LP vault shares targeting
  approximately linear exposure to one market asset.

Removed concepts:

- no legacy LP-token branding;
- no fixed 1:1 protected-principal LP token;
- no protected-principal plus junior-capital split;
- no separate fee-eligibility step for normal LPs;
- no public reserve LP token;
- no LP-token-denominated hLP debt;
- no per-user hLP leverage position;
- no V1/V2 swap router.

## Public Instructions

V2 uses simple action names inside the standalone program:

```text
initialize
update_config
set_reduce_only
add_liquidity
remove_liquidity
set_yield_recipient
claim_yield
swap
deposit_collateral
withdraw_collateral
borrow
repay
liquidate
open_hedge
close_hedge
```

Futarchy and revenue administration mirrors the V1 flow:

```text
init_futarchy_authority
update_futarchy_authority
update_protocol_revenue
update_revenue_recipients
set_global_reduce_only
claim_protocol_fees
```

## yLP Model

Normal liquidity is always yield-bearing. A user deposits both assets at the
current market ratio and receives both side tokens:

```text
user X + Y -> market reserves -> yLP_X + yLP_Y
```

yLP is a floating pro-rata reserve share:

```text
asset_claim_i = ylp_shares_i * live_reserve_i / ylp_supply_i
```

Principal reserves exclude fee and interest vault balances. Swap fees and
borrow interest are routed to side-specific revenue vaults and growth indexes,
then claimed through `claim_yield`. Revenue does not auto-compound into
principal reserves.

Token-2022 transfer hooks checkpoint sender and receiver yield state before
transfer. `set_yield_recipient` lets treasuries or protocol-owned liquidity
route claimable revenue to an external wallet.

## hLP Model

Each market has two aggregate hLP vaults:

- `hLP_X`: user deposits X, vault borrows Y.
- `hLP_Y`: user deposits Y, vault borrows X.

hLP is a vault share, not a wrapper around one yLP side. Opening `hLP_X`:

```text
user deposits X
vault borrows Y
vault adds balanced X/Y liquidity
vault receives and locks yLP_X + yLP_Y
user receives hLP_X
```

Closing `hLP_X`:

```text
user burns hLP_X
vault burns proportional yLP_X + yLP_Y
Y proceeds repay Y debt
remaining X returns to user
```

hLP debt is denominated in the borrowed underlying asset. Principal NAV is:

```text
hLP_NAV = value(vault_owned_yLP_X + vault_owned_yLP_Y) - borrowed_asset_debt
```

Fee revenue remains separate from principal NAV.

## Swap And hLP Rebalancing

`swap` is the V2 swap instruction. It snapshots risk, applies the user swap,
then checkpoints both aggregate hLP vaults in O(1). The hLP reaction uses a
bounded balanced-yLP adjustment, so the post-swap spot used for the quote is
preserved within rounding and there is no hidden second price move.

Current implementation status:

- swaps stay live and checkpoint active hLP vaults;
- active hLP swaps require the canonical hLP-owned yLP vault accounts;
- hLP rebalance deltas execute as balanced yLP mint/burn plus underlying debt
  adjustment when feasible;
- leverage-up is capped by borrowed-side cash headroom;
- unexecuted hLP rebalance is stored as `pending_rebalance` without blocking
  ordinary swaps.

Settlement guards apply to hLP mint/burn realization, not ordinary swaps.

## Risk And Settlement

V2 uses cached settlement references and liquidity EMA state so settlement
cannot rely on one-block manipulated spot. Ordinary swaps remain optimistic,
while hLP mint/burn uses conservative settlement pricing.

Daily limits are sized from liquidity EMA. Risk checks use normalized market
valuation, recognized debt-bearing collateral, and the market health floor.
Soft borrow remains disabled by default.

## Invariants

- yLP supply is backed by reserve-side principal accounting.
- No operation mints yLP without corresponding reserve value.
- Fee liabilities are backed by fee or interest vault balances.
- hLP NAV satisfies `collateral_value - debt_value >= 0`.
- hLP debt shares match the aggregate vault debt.
- hLP operations never use yLP-denominated debt.
- Swap-time hLP updates are O(1) and never iterate user positions.
- V1 pair instructions, events, seeds, and account layouts remain unchanged.

## Verification Gates

Core local gates:

```bash
anchor build -p omnipair_v2
cargo test -p omnipair-v2 --lib
npm run build --prefix packages/program-interface
yarn test-litesvm
```

Before production, add full LiteSVM coverage for the remaining unexercised V2
administration and liquidation flows, then complete the composite hLP swap
rebalancing tests.
