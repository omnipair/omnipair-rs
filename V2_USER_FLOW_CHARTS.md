# Omnipair V2 User Flow Charts

These charts summarize the current V2 implementation in `programs/omnipair-v2/src`.

## Legend

- `Market` owns two `MarketSide`s: base and quote.
- `ReserveLedger`: live reserve, cash reserve, reserved liability.
- `ClaimTokenLedger`: protected claim supply, hedged claim supply, staked claim supply.
- `BufferLedger`: buffer share supply, staked buffer shares, required buffer, buffer ratio.
- `FeeLedger`: fee vault balance plus staker, hedged, unallocated, operator, and protocol liabilities.
- `DebtBook`: market-wide fixed debt shares and borrow indexes.
- `RiskBook` and `MarketHealth`: market valuation, liquidity EMA, circuit breaker, and debt health stats.
- `RecognitionLedger`: debt-bearing recognized collateral aggregates.
- User positions are `StakePosition`, `MarginPosition`, and `HedgePosition`.

## 1. Whole System Map

```mermaid
flowchart LR
  U["User wallet"] --> A["Asset token accounts"]
  U --> C["Claim omLP accounts"]
  U --> H["Hedge h-omLP accounts"]

  M["Market"] --> B["Base MarketSide"]
  M --> Q["Quote MarketSide"]
  M --> DB["DebtBook"]
  M --> RB["RiskBook"]
  M --> MH["MarketHealth"]
  M --> RL["RecognitionLedger"]
  M --> IR["InsuranceReserve"]

  B --> BR["Base reserve vault"]
  B --> BC["Base collateral vault"]
  B --> BF["Base fee vault"]
  B --> BS["Base stake vault"]
  B --> BH["Base hedge vault"]
  B --> BL["Base ledgers"]

  Q --> QR["Quote reserve vault"]
  Q --> QC["Quote collateral vault"]
  Q --> QF["Quote fee vault"]
  Q --> QS["Quote stake vault"]
  Q --> QH["Quote hedge vault"]
  Q --> QL["Quote ledgers"]

  U --> SP["StakePosition"]
  U --> MP["MarginPosition"]
  U --> HP["HedgePosition"]
```

## 2. Market Admin Flow

```mermaid
flowchart TD
  Init["initialize"] --> Validate["Validate config, mints, manager, operator"]
  Validate --> Create["Create market, side vaults, claim mints, hedge mints"]
  Create --> InitBooks["DebtBook indexes = NAD; RiskBook slot initialized; ledgers empty"]
  InitBooks --> Created["Emit MarketCreated"]

  Update["update_config"] --> ConfigCheck["Validate new config"]
  ConfigCheck --> BufferChange{"buffer_ratio_bps changed?"}
  BufferChange -->|"yes"| Locked["Reject if active stake or fee liability exists"]
  Locked --> Recompute["Recompute required_buffer on both sides"]
  Recompute --> Floor["Reject if reserves cannot cover claim supply + required buffer"]
  BufferChange -->|"no"| Apply["Apply config fields"]
  Floor --> Apply
  Apply --> Updated["Emit MarketUpdated"]

  Reduce["set_reduce_only"] --> Flag["Set market.reduce_only"]
  Flag --> ReduceEvent["Emit MarketUpdated"]
```

## 3. LP Principal Flow

```mermaid
sequenceDiagram
  participant User
  participant AssetATA as "user asset ATA"
  participant ReserveVault as "market reserve vault"
  participant ClaimMint as "claim omLP mint"
  participant StakePos as "StakePosition"
  participant Side as "MarketSide ledgers"

  User->>AssetATA: add_liquidity(asset, deposit_amount)
  AssetATA->>ReserveVault: transfer deposit_amount
  Side->>Side: split deposit into claim_amount + buffer_amount
  ClaimMint->>User: mint claim_amount omLP
  Side->>Side: ReserveLedger live/cash += deposit_amount
  Side->>Side: protected_claim_token_supply += claim_amount
  Side->>Side: buffer_share_supply += buffer_amount
  Side->>Side: required_buffer = f(protected_claim_supply, buffer_ratio)
  Side->>StakePos: available_buffer_share_amount += buffer_amount
  Side-->>User: emit LiquidityAdded

  User->>ClaimMint: remove_liquidity(asset, claim_amount)
  ClaimMint->>ClaimMint: burn claim_amount omLP
  ReserveVault->>User: transfer claim_amount asset
  Side->>Side: ReserveLedger live/cash -= claim_amount
  Side->>Side: protected_claim_token_supply -= claim_amount
  Side->>Side: required_buffer = f(new protected_claim_supply, buffer_ratio)
  Side->>Side: enforce reserve floor: live_reserve >= claims + required_buffer
  Side-->>User: emit LiquidityRemoved
```

Key hindsight: base omLP is fixed principal. Deposits mint only the protected claim amount; buffer shares are junior accounting credited to `StakePosition`, not transferable principal.

## 4. Fee Eligibility And Fee Claims

```mermaid
sequenceDiagram
  participant User
  participant ClaimATA as "user claim omLP ATA"
  participant StakeVault as "stake vault"
  participant StakePos as "StakePosition"
  participant FeeVault as "fee vault"
  participant Side as "MarketSide FeeLedger"

  User->>ClaimATA: stake(claim_amount, buffer_share_amount)
  Side->>Side: carry forward unallocated staker fees if active units exist
  StakePos->>StakePos: accrue fees to checkpoint
  ClaimATA->>StakeVault: transfer claim_amount
  StakePos->>StakePos: available_buffer -= buffer_share_amount
  StakePos->>StakePos: staked_claim += claim_amount; staked_buffer += buffer_share_amount
  Side->>Side: staked_claim_token_supply += claim_amount
  Side->>Side: staked_buffer_share_amount += buffer_share_amount
  Side-->>User: emit MarketStakeUpdated

  User->>StakePos: unstake(claim_amount, buffer_share_amount)
  Side->>Side: carry forward unallocated staker fees
  StakePos->>StakePos: accrue fees to checkpoint
  StakeVault->>ClaimATA: transfer claim_amount
  StakePos->>StakePos: staked_claim -= claim_amount; staked_buffer -= buffer_share_amount
  StakePos->>StakePos: available_buffer += buffer_share_amount
  Side->>Side: staked supply counters decrease
  Side-->>User: emit MarketStakeUpdated

  User->>StakePos: claim_fees(asset)
  Side->>Side: carry forward unallocated staker fees
  StakePos->>StakePos: accrue fees to checkpoint
  FeeVault->>User: transfer accrued_fee_amount
  Side->>Side: fee_liability -= claimed amount
  StakePos->>StakePos: accrued_fee_amount = 0
  Side-->>User: emit MarketFeesClaimed

  User->>Side: claim_market_fees(operator or protocol)
  FeeVault->>User: transfer selected market fee bucket
  Side->>Side: operator_fee_liability or protocol_fee_liability = 0
  Side-->>User: emit MarketFeeLiabilityClaimed
```

Key hindsight: fees do not rebase omLP. Swap fees create liabilities in `FeeLedger`; staked matched claim plus buffer receives fee index growth.

## 5. Swap Flow And Fee Routing

```mermaid
flowchart TD
  Start["swap(asset_in, exact_asset_in, min_asset_out)"] --> TransferIn["Trader sends asset_in to reserve_in and fee vault receives fee_credit"]
  TransferIn --> Quote["Compute amount_in_after_fee and amount_out"]
  Quote --> Floor["Check reserve_out post-state covers protected claims + required buffer"]
  Floor --> Reserves["reserve_in live/cash += amount_in_after_fee; reserve_out live/cash -= amount_out"]
  Reserves --> FeeSplit["RecordFeeCredit on input side"]
  FeeSplit --> Op["operator_fee_liability += operator cut"]
  FeeSplit --> Proto["protocol_fee_liability += protocol cut"]
  FeeSplit --> LP["LP fee = fee_credit - operator - protocol"]
  LP --> Route{"hedged supply and routing K"}
  Route -->|"free LP"| Staker["unallocated_fee_liability or fee_growth_index_nad"]
  Route -->|"hedged LP"| Hedged["unallocated_hedged_fee_liability or hedged_fee_growth_index_nad"]
  Staker --> Backed["fee_vault_balance backs all liabilities"]
  Hedged --> Backed
  Backed --> TransferOut["Trader receives asset_out"]
  TransferOut --> Event["Emit SwapExecuted"]
```

## 6. Borrowing, Repay, And Collateral Flow

```mermaid
flowchart TD
  Deposit["deposit_collateral(asset, amount)"] --> D1["User asset ATA -> collateral vault"]
  D1 --> D2["MarginPosition base_collateral or quote_collateral += collateral_credit"]
  D2 --> D3["Emit MarketCollateralDeposited"]

  Borrow["borrow(debt_asset, amount)"] --> B1["Recognize debt-bearing opposite collateral"]
  B1 --> B2["Apply recognized_collateral_cap_bps"]
  B2 --> B3["DailyLimitBook.borrowed_bucket += amount after decay"]
  B3 --> B4["Check debt side reserve floor and cash headroom"]
  B4 --> B5["Debt side ReserveLedger live/cash -= borrow_amount"]
  B5 --> B6["DebtBook fixed debt shares += new shares"]
  B6 --> B7["MarginPosition fixed debt shares += new shares"]
  B7 --> B8["RecognitionLedger aggregate += recognized collateral delta"]
  B8 --> B9["refresh RiskBook and MarketHealth"]
  B9 --> B10["Check market health and position health"]
  B10 --> B11["Reserve vault -> borrower debt asset ATA"]
  B11 --> B12["Emit MarketDebtUpdated and MarketHealthUpdated"]

  Repay["repay(debt_asset, amount)"] --> R1["Borrower debt asset ATA -> reserve vault"]
  R1 --> R2["Burn proportional fixed debt shares"]
  R2 --> R3["Release recognized collateral proportionally"]
  R3 --> R4["Debt side ReserveLedger live/cash += repay_credit"]
  R4 --> R5["DebtBook and MarginPosition fixed debt shares decrease"]
  R5 --> R6["RecognitionLedger aggregate decreases"]
  R6 --> R7["refresh MarketHealth"]
  R7 --> R8["Emit MarketDebtUpdated and MarketHealthUpdated"]

  Withdraw["withdraw_collateral(asset, amount)"] --> W1["Only idle collateral can leave"]
  W1 --> W2["DailyLimitBook.withdrawn_bucket += amount after decay"]
  W2 --> W3["MarginPosition collateral -= amount"]
  W3 --> W4["Collateral vault -> user asset ATA"]
  W4 --> W5["refresh MarketHealth"]
  W5 --> W6["Emit MarketCollateralWithdrawn"]
```

Key hindsight: idle collateral does not support market health. Borrow converts capped opposite-side collateral into recognized debt-bearing collateral.

## 7. Liquidation And Insurance Flow

```mermaid
flowchart TD
  Fund["deposit_insurance(asset, amount)"] --> F1["Sponsor asset ATA -> insurance vault"]
  F1 --> F2["InsuranceReserve base_available or quote_available += credit"]
  F2 --> F3["Emit MarketInsuranceFunded"]

  Liq["liquidate(debt_asset, repay_credit, max_socialized_loss)"] --> L1["Read borrower debt and opposite collateral"]
  L1 --> L2["Compute collateral_to_seize"]
  L2 --> L3{"collateral exhausted and bad debt remains?"}
  L3 -->|"no"| L4["Liquidator repay covers debt reduction"]
  L3 -->|"yes"| L5["Draw InsuranceReserve up to request/cap"]
  L5 --> L6{"debt still remains?"}
  L6 -->|"yes"| L7["socialized_loss must be <= max_socialized_loss"]
  L6 -->|"no"| L4
  L7 --> L8["Reduce borrower debt shares"]
  L4 --> L8
  L8 --> L9["Seize borrower collateral"]
  L9 --> L10["Decrease recognized collateral and RecognitionLedger"]
  L10 --> L11["Debt reserve live/cash += repay_credit + insurance_credit"]
  L11 --> L12["InsuranceReserve available -= insurance_spent"]
  L12 --> L13["refresh MarketHealth and risk circuit breakers"]
  L13 --> L14["Emit PositionLiquidated"]
```

## 8. Hedged LP Wrapper Flow

```mermaid
sequenceDiagram
  participant User
  participant ClaimATA as "user claim omLP ATA"
  participant HedgeMint as "h-omLP mint"
  participant HedgeVault as "hedge vault"
  participant HedgePos as "HedgePosition"
  participant Side as "MarketSide ledgers"
  participant Market as "Market health"

  User->>ClaimATA: open_hedge(asset, claim_amount)
  Side->>Side: carry forward unallocated hedged fees
  HedgePos->>HedgePos: accrue hedged fees to checkpoint
  ClaimATA->>HedgeVault: transfer claim_amount omLP
  HedgeMint->>User: mint hedge_amount h-omLP
  Side->>Side: hedged_claim_token_supply += claim_amount
  HedgePos->>HedgePos: hedged_claim_token_amount += claim_amount
  Market->>Market: refresh health and assert floor
  Side-->>User: emit MarketHedgeOpened

  User->>HedgeMint: close_hedge(asset, hedge_amount)
  Side->>Side: carry forward unallocated hedged fees
  HedgePos->>HedgePos: accrue hedged fees to checkpoint
  HedgeMint->>HedgeMint: burn hedge_amount h-omLP
  HedgeVault->>User: transfer claim_amount omLP
  Side->>Side: hedged_claim_token_supply -= hedge_amount
  HedgePos->>HedgePos: hedged_claim_token_amount -= hedge_amount
  Market->>Market: refresh health
  Side-->>User: emit MarketHedgeClosed

  User->>HedgePos: claim_hedge_fees(asset)
  Side->>Side: carry forward unallocated hedged fees
  HedgePos->>HedgePos: accrue hedged fees
  Side->>User: fee vault transfers accrued hedged fees
  Side->>Side: hedged_fee_liability -= claimed amount
  Side-->>User: emit MarketHedgeFeesClaimed
```

Key hindsight: h-omLP is an overlay wrapper around base omLP. It unwraps back into claim omLP and has separate hedged fee accounting.

## 9. Health, Limits, And Circuit Breakers

```mermaid
flowchart TD
  AnyRisk["Risk-increasing or debt-changing path"] --> Refresh["refresh_market_health"]
  Refresh --> Risk["RiskBook refreshes market-native value, liquidity EMA, K/price state"]
  Risk --> Debt["Effective debt = fixed debt + configured hedged overlay weighting"]
  Debt --> Health["MarketHealth stores recognized collateral and effective debt"]
  Health --> Gates{"Gates"}
  Gates --> H1["market_health_min_bps"]
  Gates --> H2["position min_health_bps"]
  Gates --> H3["spot/EMA divergence circuit breaker"]
  Gates --> H4["daily borrow and withdraw buckets"]
  Gates --> H5["reserve floor: live_reserve >= protected claims + required buffer"]
  H1 --> Pass["Instruction may commit"]
  H2 --> Pass
  H3 --> Pass
  H4 --> Pass
  H5 --> Pass
```

## 10. One-Line Flow Index

```mermaid
flowchart LR
  A["initialize"] --> B["MarketCreated"]
  C["update_config / set_reduce_only"] --> D["MarketUpdated"]
  E["add_liquidity"] --> F["LiquidityAdded"]
  G["remove_liquidity"] --> H["LiquidityRemoved"]
  I["stake / unstake"] --> J["MarketStakeUpdated"]
  K["claim_fees"] --> L["MarketFeesClaimed"]
  M["claim_market_fees"] --> N["MarketFeeLiabilityClaimed"]
  O["swap"] --> P["SwapExecuted"]
  Q["deposit_collateral"] --> R["MarketCollateralDeposited"]
  S["withdraw_collateral"] --> T["MarketCollateralWithdrawn"]
  U["borrow / repay"] --> V["MarketDebtUpdated + MarketHealthUpdated"]
  W["deposit_insurance"] --> X["MarketInsuranceFunded"]
  Y["liquidate"] --> Z["PositionLiquidated"]
  AA["open_hedge"] --> AB["MarketHedgeOpened"]
  AC["close_hedge"] --> AD["MarketHedgeClosed"]
  AE["claim_hedge_fees"] --> AF["MarketHedgeFeesClaimed"]
```

