# Flash Loan Receiver Example

Example program that receives and handles flash loans from Omnipair.

## What It Does

This program:
- Receives flash loan callback from Omnipair via CPI
- Has access to borrowed tokens
- Can execute custom strategy (arbitrage, liquidation, etc.)
- Returns tokens back to Omnipair vaults

## Building

Builds automatically with the workspace:

```bash
anchor build
```

## Deploying

```bash
anchor deploy
```

## Testing

Use the test script:

```bash
yarn test-flashloan
```

## Customizing Your Strategy

Edit the section marked "YOUR STRATEGY GOES HERE" in `src/lib.rs`:

```rust
// YOUR STRATEGY GOES HERE
// Example:
// - Swap on DEX A
// - Swap on DEX B  
// - Keep the profit
```

Add any DEX accounts or other accounts you need via `remaining_accounts` when calling the flash loan.

## Important

- The callback **must** return the exact borrowed amounts
- Accounts must be in the correct order (initiator, token accounts, mints, vaults, program)
- This runs via CPI - all happens in one transaction atomically
