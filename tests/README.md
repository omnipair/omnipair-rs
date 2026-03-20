# Omnipair Unit Tests

This directory contains unit tests for the Omnipair Solana program using **LiteSVM**, a fast and lightweight Solana VM for testing.

## Overview

LiteSVM provides an in-memory Solana runtime that allows you to:
- Test smart contracts without network calls
- Run tests locally at high speed
- Debug transactions with detailed error messages
- Maintain full control over the blockchain state

## Quick Start

### Prerequisites

Ensure your environment is set up:
```bash
# Build the Omnipair program
anchor build

# Install dependencies (if not already done)
yarn install
```

### Running Tests

Run all litesvm-based tests:
```bash
yarn test-litesvm
```

Run tests with verbose output:
```bash
yarn test-litesvm -- --reporter spec
```

Run a specific test file:
```bash
yarn test-litesvm -- tests/basic.test.ts
```

Run tests matching a pattern:
```bash
yarn test-litesvm -- --grep "Futarchy"
```

## Test Structure

### Basic Test File: `basic.test.ts`

A minimal test file that verifies:
- Program loading
- Account balance initialization
- LiteSVM connection setup

**Key Components:**
```typescript
describe("Omnipair Program - Basic Tests", () => {
  // Setup: Run once before all tests
  before(async () => {
    // Initialize LiteSVM and load program
  });

  it("should have initialized the program", async () => {
    // Test assertion
  });
});
```

### Futarchy Authority Tests: `futarchy.test.ts`

Tests for futarchy authority initialization:
- PDA derivation verification
- IDL loading and inspection
- Account setup validation

## Test Helpers

The `utils/test-helpers.ts` module provides reusable functions:

### `initializeTestEnvironment(programId, config)`
Sets up a complete test environment with LiteSVM:
```typescript
const env = await initializeTestEnvironment(
  new PublicKey("Bd9Uhf5S8yzfop8cG9oqRs6jVcLtu8B4cb2gvRmtbNzk")
);

// Now you have:
// - env.svm: LiteSVM instance
// - env.connection: LiteSVM connection wrapper
// - env.provider: Anchor provider
// - env.program: Anchor program instance
// - env.deployer: Test deployer keypair
// - env.payer: Test payer keypair
```

### `findPDA(seeds, programId)`
Helper to derive Program Derived Addresses:
```typescript
const [pda, bump] = findPDA(["futarchy_authority"], programId);
```

### `createFundedKeypair(connection, amount)`
Create a keypair and airdrop SOL to it:
```typescript
const user = await createFundedKeypair(connection, 2 * LAMPORTS_PER_SOL);
```

### `formatBalance(balance)`
Convert lamports to SOL with proper formatting:
```typescript
console.log(formatBalance(5000000000)); // "5 SOL"
```

## Writing Your First Test

### Step 1: Create a Test File

Create a new file in the `tests/` directory ending with `.test.ts`:

```typescript
// tests/my-feature.test.ts
import { expect } from "chai";
import { PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { initializeTestEnvironment, findPDA, formatBalance } from "./utils/test-helpers";

describe("My Feature Tests", () => {
  let env: any;

  before(async () => {
    const OMNIPAIR_PROGRAM_ID = new PublicKey("Bd9Uhf5S8yzfop8cG9oqRs6jVcLtu8B4cb2gvRmtbNzk");
    env = await initializeTestEnvironment(OMNIPAIR_PROGRAM_ID);
  });

  it("should verify basic setup", async () => {
    const balance = await env.connection.getBalance(env.payer.publicKey);
    expect(balance).to.equal(10 * LAMPORTS_PER_SOL);
    console.log(`Payer balance: ${formatBalance(balance)}`);
  });
});
```

### Step 2: Call a Program Instruction

Example of calling an instruction:

```typescript
it("should call init_futarchy_authority", async () => {
  const [futarchyAuthority] = findPDA(["futarchy_authority"], env.programId);

  try {
    const tx = await env.program.methods
      .initFutarchyAuthority({
        authority: env.deployer.publicKey,
        // Add other required args
      })
      .accounts({
        futarchyAuthority,
        deployer: env.deployer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([env.deployer])
      .rpc();

    console.log("Transaction signature:", tx);
    expect(tx).to.not.be.undefined;
  } catch (error) {
    console.error("Error:", error.message);
    throw error;
  }
});
```

### Step 3: Verify State Changes

After a transaction, verify the state was updated:

```typescript
it("should verify state after initialization", async () => {
  const account = await env.connection.getAccountInfo(futarchyAuthority);
  expect(account).to.not.be.null;
  expect(account.owner).to.equal(env.programId);
  
  // Parse account data if needed
  const data = await env.program.account.futarchyAuthority.fetch(futarchyAuthority);
  console.log("Futarchy Authority:", data);
});
```

## Common Test Patterns

### Testing Error Conditions

```typescript
it("should fail with invalid input", async () => {
  try {
    await env.program.methods
      .someInstruction({ invalidParam: -1 })
      .accounts({
        // accounts
      })
      .rpc();
    
    expect.fail("Should have thrown an error");
  } catch (error) {
    expect(error.message).to.include("InvalidArgument");
  }
});
```

### Creating Multiple Test Accounts

```typescript
import { createFundedKeypair } from "./utils/test-helpers";

it("should work with multiple users", async () => {
  const user1 = await createFundedKeypair(env.connection, 2 * LAMPORTS_PER_SOL);
  const user2 = await createFundedKeypair(env.connection, 2 * LAMPORTS_PER_SOL);
  
  const balance1 = await env.connection.getBalance(user1.publicKey);
  const balance2 = await env.connection.getBalance(user2.publicKey);
  
  expect(balance1).to.equal(2 * LAMPORTS_PER_SOL);
  expect(balance2).to.equal(2 * LAMPORTS_PER_SOL);
});
```

### Testing with SPL Tokens

```typescript
import { createMint, createAccount, mintTo } from "@solana/spl-token";

it("should handle token transfers", async () => {
  // Create a test mint
  const mint = await createMint(
    env.connection,
    env.payer,
    env.payer.publicKey,
    null,
    6
  );
  
  // Create token accounts
  const userTokenAccount = await createAccount(
    env.connection,
    env.payer,
    mint,
    env.deployer.publicKey
  );
  
  // Mint tokens
  await mintTo(
    env.connection,
    env.payer,
    mint,
    userTokenAccount,
    env.payer,
    1_000_000 // 1 token with 6 decimals
  );
  
  // Now use in your test
});
```

## Debugging Tests

### Enable Detailed Logging

Add console logs in your tests:
```typescript
it("should debug transaction", async () => {
  console.log("Payer:", env.payer.publicKey.toString());
  console.log("Program ID:", env.programId.toString());
  
  const tx = await env.program.methods
    .someInstruction()
    .accounts({ /* ... */ })
    .rpc();
    
  console.log("Tx signature:", tx);
});
```

### Check Transaction Logs

LiteSVM provides detailed logs for failed transactions:
```typescript
try {
  await env.program.methods.someInstruction().rpc();
} catch (error) {
  console.error("Error logs:");
  console.error(error.message);
}
```

## Performance Tips

1. **Reuse Test Environment**: The `before()` hook runs once per describe block
2. **Batch Operations**: Group related tests to minimize setup overhead
3. **Use Specific Describes**: Break large test files into focused describe blocks
4. **Clean Up State**: If tests modify shared state, reset in `beforeEach()`

## File Organization

```
tests/
├── README.md                 # This file
├── basic.test.ts             # Basic setup and connectivity tests
├── futarchy.test.ts          # Futarchy authority tests
├── utils/
│   ├── litesvm-connection.ts # LiteSVM connection wrapper
│   └── test-helpers.ts       # Reusable test utilities
```

## Troubleshooting

### "Program file not found"
```
Error: Program file not found at .../target/deploy/omnipair.so
Solution: Run `anchor build` first
```

### "IDL file not found"
```
Error: IDL file not found at .../target/idl/omnipair.json
Solution: Run `anchor build` first
```

### Tests timing out
```
Increase mocha timeout in test file:
```typescript
this.timeout(10000); // 10 seconds
```
```

### Transaction failures with detailed logs
- Check error messages carefully
- Print account states before and after
- Verify all required accounts are provided
- Check instruction discriminators match IDL

## Resources

- [LiteSVM Documentation](https://github.com/LiteSVM/litesvm)
- [Anchor Documentation](https://book.anchor-lang.com/)
- [Mocha Testing Guide](https://mochajs.org/)
- [Chai Assertions](https://www.chaijs.com/api/)
- [Solana Web3.js](https://solana-labs.github.io/solana-web3.js/)

## Next Steps

1. Run `yarn test-litesvm` to verify setup
2. Review `basic.test.ts` to understand structure
3. Create your first custom test file
4. Add tests for your specific program instructions
5. Set up CI/CD to run tests automatically
