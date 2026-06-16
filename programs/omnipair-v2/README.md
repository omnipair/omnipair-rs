# Omnipair V2

Omnipair V2 is a separate market architecture program. V1 remains the legacy GAMM pair program in `programs/omnipair`; V2 owns its own market accounts, claim tokens, hedge tokens, risk books, instructions, events, and IDL surface.

## Source Boundaries

- `instructions/`: Anchor account validation, token movement, slippage checks, and event emission.
- `transitions/`: atomic accounting mutations that return receipts for events and tests.
- `state/`: account layouts, local invariants, and market/state methods.
- `tokens/`: protocol meaning and validation for externally transferable claim and hedge token mints.
- `math/`: fixed-point, AMM, EMA, valuation, and circuit-breaker helpers.
- `utils/`: remaining shared accounting helpers used across transitions.

Instruction names are clean in the V2 namespace: `initialize`, `swap`, `add_liquidity`, `remove_liquidity`, `borrow`, `repay`, `liquidate`, `stake`, `unstake`, `claim_fees`, `claim_market_fees`, `open_hedge`, `claim_hedge_fees`, and `close_hedge`.

## Core Invariants

- Claim tokens are fixed principal claims. Base claim tokens do not rebase, compound fees, or use a dynamic exchange rate.
- Reserve floors must cover protected claim token supply plus required buffer on each market side.
- Deposits split into protected claim amount and buffer share amount; only the claim amount is minted as transferable claim tokens.
- Fee rights require matched staked claim tokens plus buffer shares. Unstaked claim tokens remain principal-only redemption claims.
- Fees are non-compounding liabilities. They are routed through fee ledgers, fee growth indexes, and explicit claim paths.
- Market health uses recognized debt-bearing collateral only. Idle collateral contributes zero to borrow health.
- Fixed debt is valued in normalized debt units. Hedged overlay debt is gamma-weighted against liquidity EMA, while fixed and soft debt remain fully effective.
- Risk books roll EMA values from cached spot and liquidity observations, then store the current observation for the next refresh.
- Borrow, redeem, repay, liquidation, fee claim, and hedge close paths check risk circuit breakers where they increase or settle risk against market prices.
- Liquidation reduces only insolvent debt and follows the waterfall: borrower collateral, liquidator repayment and incentive, insurance reserve, then LP socialization.
- Hedge tokens wrap base claim tokens one-to-one and unwrap back into base claims. They do not create staking rights.
- Inventory-native settlement is used for reserves, collateral, fees, insurance, claims, and hedge vaults.

## Verification

Useful focused checks while changing V2:

```bash
cargo test -p omnipair-v2 --lib -- --nocapture
cargo check -p omnipair-v2
anchor build -p omnipair-v2
yarn test-litesvm --grep "Omnipair Market LiteSVM"
```

Run package interface builds when public IDL, account, event, seed, or instruction shapes change.
