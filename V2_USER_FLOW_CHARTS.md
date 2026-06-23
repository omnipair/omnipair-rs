# Omnipair V2 yLP / hLP User Flow Charts

Brief implementation map for the final V2 model.

## Core State

```mermaid
flowchart LR
  Market["Market"]
  Market --> Reserves["Reserves\nlive/cash accounting"]
  Market --> Shares["ReserveShares\nyLP supplies"]
  Market --> Fees["Fees\nswap + interest indexes"]
  Market --> Debt["Debt\nfixed debt shares + indexes"]
  Market --> Risk["Risk\nspot, EMA, health, circuit checks"]
  Market --> Limits["DailyLimits\nborrow/withdraw buckets"]
  Market --> BaseHlp["base HlpVault"]
  Market --> QuoteHlp["quote HlpVault"]

  BaseHlp --> BaseOwnedYlp["owned base yLP"]
  BaseHlp --> QuoteOwnedYlp["owned quote yLP"]
  BaseHlp --> BorrowedQuote["quote debt shares"]

  QuoteHlp --> QuoteOwnedYlp2["owned quote yLP"]
  QuoteHlp --> BaseOwnedYlp2["owned base yLP"]
  QuoteHlp --> BorrowedBase["base debt shares"]
```

## Market Initialization

```mermaid
flowchart TD
  Admin["deployer / authority"] --> Futarchy["init_futarchy_authority"]
  Futarchy --> Init["initialize"]
  Init --> Market["Market PDA"]
  Init --> Vaults["reserve, collateral, insurance,\nfee, interest vaults"]
  Init --> Ylp["Token-2022 yLP base + quote mints\ntransfer hook = V2 program"]
  Init --> Hlp["Token-2022 hLP base + quote mints\ntransfer hook = V2 program"]
  Init --> FeeFlow["V1-style futarchy revenue recipients"]
```

## Normal LP: Add / Remove Liquidity

```mermaid
sequenceDiagram
  participant User
  participant Market
  participant ReserveVaults as Reserve vaults
  participant YlpMints as yLP mints
  participant Yield as Yield accounts

  User->>Market: add_liquidity(base_in, quote_in)
  Market->>Yield: checkpoint existing yLP revenue
  User->>ReserveVaults: transfer base + quote
  Market->>Market: calculate floating reserve-share mint amounts
  YlpMints->>User: mint yLP_base + yLP_quote
  Market->>Market: update reserves, yLP supplies, risk

  User->>Market: remove_liquidity(base_yLP, quote_yLP)
  Market->>Yield: checkpoint existing yLP revenue
  YlpMints->>YlpMints: burn matched yLP shares
  ReserveVaults->>User: transfer pro-rata base + quote reserves
  Market->>Market: update reserves, supplies, risk, daily limits
```

Key point: yLP is a floating reserve-side share. There is no fixed 1:1 claim and no buffer token.

## yLP Revenue

```mermaid
flowchart LR
  Swap["swap fee in token i"] --> FeeVault["side fee vault"]
  Interest["borrow interest in token i"] --> InterestVault["side interest vault"]
  FeeVault --> Index["swap_fee_growth_i"]
  InterestVault --> Index2["interest_growth_i"]
  Holder["yLP_i holder"] --> YieldAccount["YieldAccount(owner, market, i, yLP)"]
  Index --> YieldAccount
  Index2 --> YieldAccount
  YieldAccount --> Claim["claim_yield"]
  Claim --> Recipient["owner or designated recipient"]
```

Revenue is non-compounding. It is claimable separately and does not rebase principal reserves.

## hLP Open / Close

```mermaid
sequenceDiagram
  participant User
  participant HlpVault as hLP target vault
  participant Market
  participant ReserveVaults as reserve vaults
  participant YlpMints as yLP mints
  participant HlpMint as hLP mint

  User->>HlpVault: open_hedge(target asset deposit)
  HlpVault->>Market: borrow opposite asset
  HlpVault->>ReserveVaults: add balanced target + borrowed liquidity
  YlpMints->>HlpVault: mint both yLP sides to vault-owned accounts
  HlpMint->>User: mint hLP target shares
  Market->>Market: target about 2x LP collateral / 50% debt

  User->>HlpVault: close_hedge(hLP burn)
  HlpMint->>HlpMint: burn user hLP
  HlpVault->>YlpMints: burn proportional owned yLP
  ReserveVaults->>HlpVault: withdraw underlying reserves
  HlpVault->>Market: repay borrowed-side debt
  HlpVault->>User: return remaining target asset
```

hLP is a vault share over aggregate 2x LP leverage. Debt is denominated in the borrowed underlying asset, never in yLP.

## Swap With O(1) hLP Rebalancing

```mermaid
flowchart TD
  Quote["User asks for swap quote"] --> Sim["Simulate user swap"]
  Sim --> Mark["Mark base and quote hLP vaults at post-swap spot"]
  Mark --> Delta["delta = collateral_value - 2 * debt_value"]
  Delta --> Net["Net opposing hLP flows if possible"]
  Net --> Solve["Bounded deterministic composite transition"]
  Solve --> Apply["Apply one atomic swap + hLP checkpoint"]
  Apply --> Out["User receives quoted output"]
  Apply --> Pending["Store pending_rebalance if hard liquidity blocks full target"]
  Apply --> Risk["Refresh risk / EMA after transition"]
```

The user quote includes the hLP reaction. There is no hidden post-swap rebalance.

## Borrow / Repay

```mermaid
sequenceDiagram
  participant User
  participant Market
  participant Collateral as collateral vault
  participant Reserve as reserve vault
  participant Position as MarginPosition

  User->>Market: deposit_collateral(asset, amount)
  User->>Collateral: transfer collateral
  Market->>Position: record deposited collateral

  User->>Market: borrow(debt_asset, collateral_asset, amount)
  Market->>Market: value recognized collateral using risk book
  Market->>Market: enforce health, daily limit, circuit checks
  Reserve->>User: transfer borrowed asset
  Market->>Position: add fixed debt shares

  User->>Market: repay(debt_asset, amount)
  User->>Reserve: transfer repayment
  Market->>Position: reduce fixed debt shares
```

Idle collateral does not pump market health. Borrowing uses recognized collateral and fixed underlying-token debt.

## Liquidation

```mermaid
flowchart TD
  Check["liquidate"] --> Health["Revalue collateral and debt"]
  Health --> Bad{"position unhealthy?"}
  Bad -->|"no"| Reject["reject"]
  Bad -->|"yes"| Repay["liquidator repays debt asset"]
  Repay --> Seize["seize collateral at penalty"]
  Seize --> Liquidator["1% incentive to liquidator"]
  Seize --> Insurance["2% penalty to insurance reserve"]
  Seize --> BadDebt{"bad debt remains?"}
  BadDebt -->|"yes"| Waterfall["insurance first, then LP socialization"]
  BadDebt -->|"no"| Done["position debt reduced / closed"]
```

Solvent liquidation penalty is split 1% liquidator and 2% insurance reserve.

## Transfer Hooks

```mermaid
sequenceDiagram
  participant Sender
  participant Token2022
  participant Hook as V2 transfer hook
  participant Source as source YieldAccount
  participant Dest as destination YieldAccount

  Sender->>Token2022: transfer yLP or hLP
  Token2022->>Hook: execute transfer hook
  Hook->>Source: checkpoint revenue before balance decreases
  Hook->>Dest: checkpoint revenue before balance increases
  Hook-->>Token2022: approve transfer continuation
```

Transfers require the canonical yield accounts so fee indexes cannot be bypassed.
