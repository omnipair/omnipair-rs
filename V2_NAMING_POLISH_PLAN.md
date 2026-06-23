# Omnipair V2 Naming Polish Plan

V2 naming should match the final yLP / hLP architecture and the standalone
program surface.

## Keep

- `Market` for the top-level V2 account.
- `MarketSide` for base/quote side accounting.
- `Reserves`, `Shares`, `Fees`, `Debt`, `Risk`, and `DailyLimits` for compact
  state modules.
- `YieldAccount` for owner-level revenue checkpoints and recipients.
- `HlpVault` for aggregate hedged LP vault state.
- `Insurance` for liquidation loss absorption.

## Public Product Terms

- `yLP` means yield LP, a floating reserve-side LP share.
- `hLP` means hedged LP, an aggregate 2x leveraged LP vault share.
- Example symbols:
  - `yBTC-USDC`
  - `yUSDC-BTC`
  - `hBTC-USDC`
  - `hUSDC-BTC`

## Avoid In V2

- legacy LP-token branding
- fixed 1:1 protected-principal language
- public reserve LP token language
- public fee-eligibility instructions for normal LP
- retained junior-capital language
- pair or pool naming for V2 public APIs
- forced market prefixes on simple standalone program instructions

## Instruction Naming

Because V2 is its own program, prefer simple action names:

```text
add_liquidity
remove_liquidity
set_yield_recipient
claim_yield
swap
borrow
repay
liquidate
open_hedge
close_hedge
```

Do not use workaround names such as versioned swap aliases or forced
market-prefixed swap names.

## Event Naming

Prefer user-facing event names:

- `LiquidityAdded`
- `LiquidityRemoved`
- `SwapExecuted`
- `YieldClaimed`
- `HlpOpened`
- `HlpClosed`
- `HlpRebalanced`
- `PositionLiquidated`
- `MarketHealthUpdated`

## SDK Helpers

V2 helper names should describe market assets directly:

- `deriveMarketAddress`
- `deriveMarketReserveVaultAddress`
- `deriveYlpMintAddress` if mints become PDA-derived later
- `deriveHlpMintAddress` if mints become PDA-derived later
- `deriveHlpYlpVaultAddress`
- `deriveYieldAccountAddress`
- `deriveMarginPositionAddress`

Keep V1 `Pair` helpers available for legacy integrations.
