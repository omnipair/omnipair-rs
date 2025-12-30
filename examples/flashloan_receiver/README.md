# Flash Loan Receiver Example

Example program demonstrating how to implement a flash loan receiver for Omnipair.

## 📊 Flash Loan Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        USER TRANSACTION                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. User calls flashloan()                                       │
│     ├─ amount0: 1,000 tokens                                     │
│     ├─ amount1: 0 tokens                                         │
│     └─ receiverProgram: Your Program ID                          │
│                           │                                       │
│                           ▼                                       │
│  ┌────────────────────────────────────────────────┐             │
│  │      OMNIPAIR FLASH LOAN INSTRUCTION            │             │
│  ├────────────────────────────────────────────────┤             │
│  │ 2. Update pair state                            │             │
│  │ 3. Validate amounts                             │             │
│  │ 4. Record vault balances (before)               │             │
│  │ 5. Transfer tokens to receiver                  │             │
│  │    └─> token0: vault → user (1,000 tokens)     │             │
│  │                           │                      │             │
│  │                           ▼                      │             │
│  │ 6. ┌──────────────────────────────────────┐    │             │
│  │    │  CPI TO RECEIVER PROGRAM              │    │             │
│  │    ├──────────────────────────────────────┤    │             │
│  │    │  YOUR STRATEGY EXECUTES HERE:         │    │             │
│  │    │  • Swap on DEX A                      │    │             │
│  │    │  • Swap on DEX B                      │    │             │
│  │    │  • Arbitrage profit: +50 tokens       │    │             │
│  │    │  • Return tokens to vaults            │    │             │
│  │    │    └─> user → vault (1,000 tokens)    │    │             │
│  │    └──────────────────────────────────────┘    │             │
│  │                           │                      │             │
│  │                           ▼                      │             │
│  │ 7. CPI returns (success/fail)                   │             │
│  │ 8. Reload vault accounts                        │             │
│  │ 9. Verify balances restored                     │             │
│  │    ├─ token0_vault >= balance_before ✓         │             │
│  │    └─ token1_vault >= balance_before ✓         │             │
│  │ 10. Emit FlashloanEvent                         │             │
│  └────────────────────────────────────────────────┘             │
│                                                                   │
│  ✅ Transaction Success                                          │
│  💰 User keeps profit (50 tokens)                               │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘

Note: Everything happens atomically. If tokens aren't returned, 
      the entire transaction fails and reverts.
```

## 🚀 Quick Start

### 1. Build
```bash
anchor build
```

### 2. Deploy
```bash
yarn deploy-receiver
# or
anchor deploy -p flashloan_receiver_example
```

### 3. Test
```bash
yarn test-flashloan
```

## 📝 Implementation Guide

### Receiver Program Structure

Your receiver must implement a handler matching this signature:

```rust
pub fn flash_loan_callback(
    ctx: Context<FlashLoanCallback>,
    callback_data: FlashLoanCallbackData,
) -> Result<()> {
    // 1. Execute your strategy
    your_arbitrage_logic(&ctx, callback_data.amount0, callback_data.amount1)?;
    
    // 2. Return tokens to vaults (REQUIRED)
    transfer_back_to_vault(&ctx, callback_data.amount0, callback_data.amount1)?;
    
    Ok(())
}
```

### Required Accounts (in order)

```rust
#[derive(Accounts)]
pub struct FlashLoanCallback<'info> {
    pub initiator: Signer<'info>,
    #[account(mut)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub receiver_token1_account: Account<'info, TokenAccount>,
    pub token0_mint: Box<Account<'info, Mint>>,
    pub token1_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub token0_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token1_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
```

### Callback Data Structure

```rust
pub struct FlashLoanCallbackData {
    pub initiator: Pubkey,    // Who called the flash loan
    pub amount0: u64,          // Amount of token0 borrowed
    pub amount1: u64,          // Amount of token1 borrowed
    pub data: Vec<u8>,         // Custom data for your strategy
}
```

## 🎯 Strategy Examples

### Arbitrage
```rust
// 1. Borrow 1000 USDC from Omnipair
// 2. Sell on DEX A for 1.05 SOL
// 3. Buy on DEX B with 1.05 SOL → get 1050 USDC
// 4. Return 1000 USDC to Omnipair
// 5. Keep 50 USDC profit ✓
```

### Liquidation
```rust
// 1. Borrow tokens needed for liquidation
// 2. Liquidate undercollateralized position
// 3. Receive liquidation bonus (5-10%)
// 4. Return borrowed amount
// 5. Keep bonus as profit ✓
```

### Collateral Swap
```rust
// 1. Borrow token A
// 2. Repay your existing debt
// 3. Withdraw your collateral (token B)
// 4. Swap B for A on DEX
// 5. Return A to flash loan
// 6. Successfully swapped collateral ✓
```

## 🔧 Customizing Your Strategy

Edit `src/lib.rs` in the marked section:

```rust
// YOUR STRATEGY GOES HERE
// Example:
// - Swap on DEX A
// - Swap on DEX B
// - Keep the profit

// Add any DEX accounts via remaining_accounts when calling flash loan
```

## 📞 Calling from TypeScript

```typescript
const tx = await omnipairProgram.methods
    .flashloan({
        amount0: new BN(1_000_000),
        amount1: new BN(0),
        data: Buffer.from([]),
    })
    .accountsPartial({
        pair: pairPda,
        rateModel: rateModel,
        token0Vault: token0Vault,
        token1Vault: token1Vault,
        token0Mint: TOKEN0_MINT,
        token1Mint: TOKEN1_MINT,
        receiverToken0Account: userToken0Account,
        receiverToken1Account: userToken1Account,
        receiverProgram: RECEIVER_PROGRAM_ID,
        user: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
    })
    .remainingAccounts([
        // Vaults for returning tokens
        { pubkey: token0Vault, isSigner: false, isWritable: true },
        { pubkey: token1Vault, isSigner: false, isWritable: true },
        // Add your DEX accounts here
        // { pubkey: dexPool, isSigner: false, isWritable: true },
    ])
    .rpc();
```

## ⚠️ Important Notes

1. **Return Tokens**: Your callback MUST return the exact borrowed amounts before completing
2. **Account Order**: Accounts must be in the exact order shown above
3. **Atomicity**: Everything happens in one transaction. Failure = full revert
4. **Remaining Accounts**: Pass vaults + any DEX accounts you need
5. **No Fees**: Currently no fees (configurable)

## 🔒 Security

- ✅ Atomic execution (single instruction)
- ✅ Balance verification before/after
- ✅ CPI isolation
- ✅ Cannot borrow more than reserves
- ⚠️ Users should only call trusted receiver programs

## 🐛 Troubleshooting

### "Insufficient balance to return"
→ Your strategy consumed tokens. Ensure you return exact borrowed amounts.

### "Account not found"  
→ Check `.env` has correct TOKEN0_MINT and TOKEN1_MINT

### "Insufficient vault balance"
→ Add liquidity first: `yarn bootstrap`

### "Program not found"
→ Deploy receiver: `yarn deploy-receiver`

## 📚 Additional Resources

- Test script: `scripts/test_flashloan.ts`
- Main implementation: `programs/omnipair/src/instructions/lending/flashloan.rs`
- Example receiver: `examples/flashloan_receiver/src/lib.rs`

## 🎓 Learn More

Common use cases:
- **Arbitrage**: Price differences across DEXs
- **Liquidations**: Liquidate positions for bonus
- **Debt Refinancing**: Move debt to better rates
- **Collateral Swaps**: Change collateral type atomically

---

**Ready to build?** Start by customizing the strategy in `src/lib.rs`! 🚀