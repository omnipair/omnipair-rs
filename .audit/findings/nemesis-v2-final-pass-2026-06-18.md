# Nemesis V2 Final Pass - 2026-06-18

## Scope

- Program: `programs/omnipair-v2`
- Language/framework: Rust, Anchor, SPL Token, Token-2022
- Entry points analyzed: all 19 standalone V2 instructions
- Focus: reserve floors, fee liabilities, recognized collateral/debt, cached
  EMA risk book, daily limits, insurance/liquidation waterfall, stake and hedge
  supply coupling, admin/reduce-only authority paths

## Coupled State Map

| Coupled state | Required invariant | Primary mutation paths |
| --- | --- | --- |
| reserve ledger, claim token ledger, buffer ledger | live reserves cover protected claims plus required buffer | `add_liquidity`, `remove_liquidity`, `swap`, `borrow`, `repay`, `liquidate` |
| fee vault balance, fee liabilities, fee indexes | every fee liability bucket is backed and has a settlement path | `swap`, `claim_fees`, `claim_hedge_fees`, `claim_market_fees`, `stake` |
| fixed debt shares, margin position debt, recognition ledger | aggregate debt/recognition equals the sum of active debt-bearing positions | `borrow`, `repay`, `liquidate`, collateral withdrawal checks |
| risk book cached observations, EMA values, market health | EMA rolls from cached prior observations, then stores current observations | risk refreshes in swap, borrow, repay, withdraw, liquidation, fee claim, hedge paths |
| staked claim supply, staked buffer shares, stake positions | fee-eligible units require matched claim tokens and buffer shares | `stake`, `unstake`, `claim_fees`, reserve deposits |
| hedged claim supply, hedge positions, hedge vaults | h-omLP wraps claim tokens 1:1 and does not grant staking rights | `open_hedge`, `close_hedge`, `claim_hedge_fees` |
| reduce-only flag, operator, emergency authority | incident response can block risk-increasing paths | `set_reduce_only`, `assert_live`-guarded instructions |

## Verified Finding

### NM-V2-FINAL-001: Emergency Reduce-Only Authority Was Unreachable

**Severity:** Medium
**Status:** Fixed in `554b7bf fix(v2): allow emergency reduce-only authority`

**Coupled pair:** `REDUCE_ONLY_EMERGENCY_AUTHORITY` intent and
`set_reduce_only` authorization.

**Invariant:** If the program declares an emergency signer authorized to toggle
reduce-only mode, that signer must be able to reach the reduce-only transition.

**Breaking operation:** `set_reduce_only`

Before the fix, V2 carried the same emergency reduce-only constant and error
surface as V1, but the V2 instruction accepted only `market.operator`:

- `programs/omnipair-v2/src/constants.rs:45`
- `programs/omnipair-v2/src/instructions/market/set_reduce_only.rs:30`

If the operator key was unavailable or compromised during an incident, the
configured emergency signer could not activate reduce-only mode.

**Fix:**

- `programs/omnipair-v2/src/lib.rs:50` now runs account validation on
  `set_reduce_only`.
- `programs/omnipair-v2/src/instructions/market/set_reduce_only.rs:30` exposes
  the signer as `authority`.
- `programs/omnipair-v2/src/instructions/market/set_reduce_only.rs:56` accepts
  either `market.operator` or `REDUCE_ONLY_EMERGENCY_AUTHORITY`.
- V2 IDL/types and LiteSVM callers use the new `authority` account name.
- V2 README and release checklist document the emergency authority path.

**Verification:**

- `cargo check -p omnipair-v2 --lib`
- `cargo test -p omnipair-v2 --lib -- --nocapture`
- `anchor build -p omnipair-v2`
- `npm run build --prefix packages/program-interface`
- `yarn test-litesvm`

## False Positives Eliminated

| Suspect | Verdict |
| --- | --- |
| Transfer-fee assets desync reserve/collateral/fee accounting | Incoming Token-2022 paths use measured vault credits; claim and hedge mints are required fee-free. |
| Cached EMA can bootstrap from same-instruction manipulated spot | Swap and liquidity-add paths refresh the risk book before mutation and tests cover the cached-spot behavior. |
| Buffer ratio changes can reprice active stake or carried fees | Config update rejects buffer-ratio changes while active stake, staker fee liability, or carried no-stake LP fees exist. |
| Config updates can make existing debt unhealthy | Fixed in `950c7c9 fix(v2): preserve health across config updates`; config updates now refresh risk/health under the new parameters and reject if existing effective debt falls below the configured health floor. |
| Liquidation leaves debt and recognition ledgers stale | Liquidation burns debt shares, decreases position and aggregate recognition, refreshes market health, and tests cover insurance and socialization. |
| h-omLP wrappers create staking rights | Hedge wrappers escrow claim tokens 1:1 and track hedged fee exposure without changing staked claim or buffer supply. |

## Remaining Non-Code Decisions

- Keep soft borrow and soft liquidation disabled until a separate reviewed spec
  is merged.
- Run external/security-team signoff against the final release candidate before
  mainnet readiness is declared.
