# Omnipair V2

Omnipair V2 is a standalone market program that lives beside the legacy V1 pair program in `programs/omnipair`. V1 compatibility stays unchanged. V2 uses market terminology, floating yield LP shares, and aggregate hedged LP vault accounting.

## Source Boundaries

- `instructions/`: Anchor account validation, inventory movement, slippage checks, and events.
- `transitions/`: atomic accounting mutations with small receipts for events and tests.
- `state/`: account layouts, embedded market books, and invariants.
- `tokens/`: validation for Token-2022 yLP and hLP mints.
- `math/`: fixed-point, GAMM, EMA, valuation, and circuit-breaker helpers.
- `utils/`: shared accounting helpers used by transitions.

Instruction modules are split by domain: `market`, `reserve`, `yielding`, `spot`, `lending`, `liquidation`, `hedge`, and `futarchy`.

## Public Instructions

V2 exposes the current market instruction set:

- `initialize`, `update_config`, `set_reduce_only`
- `add_liquidity`, `remove_liquidity`
- `set_yield_recipient`, `claim_yield`
- `swap`
- `deposit_collateral`, `withdraw_collateral`, `borrow`, `repay`, `liquidate`
- `open_hedge`, `close_hedge`
- V1-style futarchy/revenue administration: `init_futarchy_authority`, `update_futarchy_authority`, `update_protocol_revenue`, `update_revenue_recipients`, `set_global_reduce_only`, `claim_protocol_fees`

## Token Model

Each market records three Token-2022 LP mints:

- `yLP`: the normal two-sided LP share for balanced base/quote liquidity.
- `hLP_base`: one-sided hedged LP shares targeting base exposure.
- `hLP_quote`: one-sided hedged LP shares targeting quote exposure.

yLP and hLP mints must be fee-free Token-2022 mints with a transfer hook configured to the V2 program, mint authority set to the market PDA, and no freeze authority. `initialize_lp_metadata` creates Metaplex metadata for each LP mint with the market PDA as update authority. Production builds additionally enforce vanity suffixes: `yLP` for yLP and `hLP` for each hLP mint. Underlying asset mints may be SPL Token or Token-2022 mints accepted by the shared mint validator.

## yLP Liquidity

`add_liquidity` is the normal LP entry. Users deposit both market assets at the current market ratio and receive one fungible `yLP` token.

yLP shares are floating two-sided principal shares:

```text
base_claim  = user_ylp_shares * base_live_reserve  / total_ylp_supply
quote_claim = user_ylp_shares * quote_live_reserve / total_ylp_supply
```

There is no fixed 1:1 protected-principal LP, no separate public fee-eligibility step, and no retained junior-capital account. `remove_liquidity` burns yLP and returns pro-rata base and quote principal reserves subject to cash availability and user slippage bounds.

Swap fees and borrow interest are non-compounding liabilities. They are held outside principal reserves in side-specific fee and interest vaults and distributed through side-specific growth indexes. `YieldAccount` stores owner checkpoints, accrued revenue, and an optional external revenue recipient for treasury or protocol-owned liquidity flows.

## hLP Vaults

Each market has two aggregate hLP vault records embedded in the `Market` account:

- `hLP_base`: user deposits base, the vault borrows quote, and the vault owns yLP.
- `hLP_quote`: user deposits quote, the vault borrows base, and the vault owns yLP.

Opening hLP:

```text
user target asset
  -> hLP vault borrows opposite asset
  -> vault adds balanced liquidity
  -> vault receives yLP
  -> user receives hLP_target
```

Closing hLP burns hLP shares, burns the vault's proportional yLP, repays the borrowed-side vault debt, and returns remaining target-side inventory to the user. hLP debt is denominated in the borrowed underlying asset and tracked on the aggregate hLP vault, not as borrower margin debt.

## Swaps And Rebalancing

`swap` is the V2 swap entry. It transfers inventory, routes swap fees to the fee vault, applies GAMM reserve movement, and checkpoints both aggregate hLP vaults in O(1).

hLP checkpointing computes NAV, attempts the spot-based leverage adjustment, records any unexecuted amount in `pending_rebalance`, and refreshes a cached settlement reference. The adjustment mints or burns balanced yLP, so the quoted post-swap spot is preserved within rounding and there is no hidden second price move after the user output. Leverage-up is capped by borrowed-side cash headroom; when cash is unavailable, ordinary swaps remain live and the gap is carried forward as pending rebalance. hLP open/close uses the cached reference to block settlement when spot has moved beyond `settlement_divergence_bps`.

## PDA Map

| Account | Seeds | SDK helper |
| --- | --- | --- |
| `Market` | `market_v2`, `base_mint`, `quote_mint`, `params_hash` | `deriveMarketAddress` / `deriveMarketV2Address` |
| Reserve vault | `market_reserve`, `market`, `asset_mint` | `deriveMarketReserveVaultAddress` |
| Collateral vault | `market_collateral`, `market`, `asset_mint` | `deriveMarketCollateralVaultAddress` |
| Swap fee vault | `market_fee`, `market`, `asset_mint` | `deriveMarketFeeVaultAddress` |
| Interest vault | `market_interest`, `market`, `asset_mint` | `deriveMarketInterestVaultAddress` |
| Margin position | `margin`, `market`, `owner` | `deriveMarginPositionAddress` |
| Yield account | `yield`, `market`, `owner`, `asset_mint`, `token_kind` | `deriveYieldAccountAddress` |
| Insurance vault | `insurance`, `market`, `asset_mint` | `deriveInsuranceAddress` |
| LP token metadata | Metaplex `metadata`, token metadata program, `lp_mint` | `deriveTokenMetadataAddress` |

yLP and hLP mints are supplied to `initialize` and validated by mint authority, decimals, Token-2022 owner, transfer hook, fee-free extension rules, no freeze authority, vanity suffix, and zero supply at market creation. LP metadata is created in follow-up `initialize_lp_metadata` calls, one mint per transaction.

## Event Surface

Indexers should consume V2 events from the standalone V2 IDL:

- `MarketCreated`, `MarketUpdated`, `MarketHealthUpdated`
- `LiquidityAdded`, `LiquidityRemoved`
- `YieldRecipientUpdated`, `YieldClaimed`, `MarketFeeLiabilityClaimed`, `ProtocolFeesClaimed`
- `SwapExecuted`, `HlpRebalanced`
- `MarketCollateralDeposited`, `MarketCollateralWithdrawn`, `MarketDebtUpdated`
- `PositionLiquidated`
- `HlpOpened`, `HlpClosed`

Every V2 event carries `MarketEventMetadata` with signer, market, and slot.

## Core Invariants

- yLP supply is backed by paired base/quote principal accounting.
- No operation mints yLP without corresponding reserve value.
- yLP principal reserves exclude fee and interest vault balances.
- Fee liabilities must be backed by fee and interest vault balances.
- hLP NAV is `collateral_value - debt_value` and must not underflow.
- hLP debt shares stay matched to aggregate hLP vault debt.
- hLP operations never use yLP-denominated debt.
- Market health uses recognized debt-bearing collateral for borrower debt; idle collateral contributes zero.
- Risk books update EMA values from cached pre-transition observations and store current observations for the next refresh.
- Liquidation follows the waterfall: borrower collateral, liquidator incentive, insurance, then bounded LP socialization.

## Verification

Useful focused checks while changing V2:

```bash
cargo fmt -p omnipair-v2 -- --check
cargo check -p omnipair-v2 --lib
cargo test -p omnipair-v2 --lib -- --nocapture
anchor build -p omnipair_v2
npm run build --prefix packages/program-interface
yarn test-litesvm
```

Run program-interface builds whenever public IDL, account, event, seed, or instruction shapes change.

## Legacy V1 Baseline

V1 remains the legacy program and is not expected to become clean as part of V2 review. As of the branch baseline, `cargo test -p omnipair --lib` has 5 known failures:

- `v1::state::rate_model::tests::test_default_matches_original_low_util`
- `v1::state::rate_model::tests::test_default_matches_original_high_util`
- `v1::state::rate_model::tests::test_faster_half_life_adjusts_quicker`
- `v1::state::rate_model::tests::test_uncapped_rate_grows_exponentially`
- `shared::gamm_math::tests::manipulation_bounded_by_ema`

Treat new V1 failures beyond that list as regressions, and keep V2 changes out of the legacy V1 instruction surface unless the change is explicitly scoped as V1 work.
