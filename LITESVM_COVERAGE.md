# LiteSVM Instruction Coverage

Track which Omnipair program instructions are tested by litesvm test suite.

## Quick Start

```bash
# Run tests with coverage report
yarn test-coverage
```

Output shows:
```
📊 INSTRUCTION COVERAGE REPORT
════════════════════════════════

✅ Covered Instructions: 2/16 (12.50%)
  ✓ initFutarchyAuthority    [1 test(s)]
    └─ should calculate correct PDA
  ✓ viewPairData             [1 test(s)]

❌ Untested Instructions: 14/16
  ✗ swap
  ✗ addCollateral
  ✗ borrow
  ... (14 more)

Coverage: 12.50% | Tests: 2/16
════════════════════════════════
```

## How It Works

The coverage tracker monitors which Omnipair instructions tests exercise:

### 1. Import the Tracker
```typescript
import { trackInstruction, getCoverageReport } from "./utils/instruction-coverage.js";
```

### 2. Track Instructions in Tests
```typescript
it("should execute a swap", async () => {
  trackInstruction("swap", "should execute a swap");
  
  // Your test code
  const result = await program.methods.swap(...).rpc();
});
```

### 3. View Report
Reports are displayed automatically after tests run, showing:
- ✅ Covered instructions with test counts
- ❌ Untested instructions
- Overall percentage coverage

## Tracked Instructions

The tracker monitors all 16 Omnipair instructions:

| Instruction | Category | Status |
|------------|----------|--------|
| `viewPairData` | View | - |
| `viewUserPositionData` | View | - |
| `initFutarchyAuthority` | Governance | - |
| `updateFutarchyAuthority` | Governance | - |
| `claimProtocolFees` | Governance | - |
| `distributeTokens` | Governance | - |
| `initialize` | Liquidity | - |
| `addLiquidity` | Liquidity | - |
| `removeLiquidity` | Liquidity | - |
| `swap` | Swap | - |
| `addCollateral` | Lending | - |
| `removeCollateral` | Lending | - |
| `borrow` | Lending | - |
| `repay` | Lending | - |
| `liquidate` | Lending | - |
| `flashloan` | Lending | - |

## Usage Examples

### Basic Tracking
```typescript
it("should swap tokens", async () => {
  trackInstruction("swap");
  
  // Test swap instruction
});
```

### With Test Name
```typescript
it("should handle slippage", async () => {
  trackInstruction("swap", "should handle slippage");
  
  // Test with explicit slippage checking
});
```

### Multiple Instructions in One Test
```typescript
it("should execute a complex transaction", async () => {
  trackInstruction("addCollateral", "complex transaction");
  trackInstruction("borrow", "complex transaction");
  
  // Test both instructions together
});
```

## Adding New Instructions

Update the `ALL_INSTRUCTIONS` array in `tests/utils/instruction-coverage.ts`:

```typescript
const ALL_INSTRUCTIONS = [
  "viewPairData",
  // ... existing instructions
  "myNewInstruction",  
];
```

## Integration with Tests

Add coverage tracking to every test file:

### 1. Import
```typescript
import { trackInstruction, getCoverageReport } from "./utils/instruction-coverage.js";
```

### 2. Track in Tests
```typescript
describe("My Feature", () => {
  it("should do something", async () => {
    trackInstruction("myInstruction", "should do something");
    // test
  });
});
```

### 3. Generate Report
```typescript
after(() => {
  getCoverageReport();
});
```

## Coverage API

### trackInstruction(instructionName, testName?)
```typescript
// Track an instruction
trackInstruction("swap");

// Track with test name for better reporting
trackInstruction("swap", "should handle slippage");
```

### getCoverageReport()
```typescript
// Display formatted coverage report
getCoverageReport();

// Returns coverage data
// {
//   covered: 5,
//   total: 16,
//   percentage: "31.25",
//   testedInstructions: ["swap", "addCollateral", ...],
//   untestedInstructions: ["borrow", "repay", ...]
// }
```

### getCoverageData()
```typescript
// Get coverage as object (without printing)
const data = getCoverageData();
console.log(`Coverage: ${data.percentage}%`);
```

### resetCoverage()
```typescript
// Clear all tracking (useful between test suites)
resetCoverage();
```

## Sample Output

```
  Omnipair Program - Swap Tests
    ✓ Tested: swap
    ✔ should swap tokens with exact amount

  Omnipair Program - Lending Tests
    ✓ Tested: addCollateral
    ✓ Tested: borrow
    ✓ Tested: repay
    ✔ should add collateral and borrow

══════════════════════════════════════════════════════════════════════
📊 INSTRUCTION COVERAGE REPORT
══════════════════════════════════════════════════════════════════════

✅ Covered Instructions: 4/16 (25.00%)

  ✓ swap                      [1 test(s)]
    └─ should swap tokens with exact amount
  ✓ addCollateral             [1 test(s)]
    └─ should add collateral and borrow
  ✓ borrow                    [1 test(s)]
    └─ should add collateral and borrow
  ✓ repay                     [1 test(s)]
    └─ should add collateral and borrow

❌ Untested Instructions: 12/16

  ✗ viewPairData
  ✗ viewUserPositionData
  ✗ initialize
  ... (9 more)

══════════════════════════════════════════════════════════════════════
Coverage: 25.00% | Tests: 4/16
══════════════════════════════════════════════════════════════════════
```

## Goal: Improving Coverage

### Current Status
- ✅ **2/16 instructions tested (12.50%)**

### Next Steps
1. Identify untested instructions from the report
2. Create tests for high-priority instructions:
   - `initialize` - Core liquidity pool setup
   - `swap` - Core DEX functionality
   - `addCollateral` - Core lending functionality
3. Re-run `yarn test-coverage` to see improvement

### Coverage Targets
- **Level 1**: 25% coverage (4/16 instructions)
- **Level 2**: 50% coverage (8/16 instructions)
- **Level 3**: 75% coverage (12/16 instructions)
- **Level 4**: 100% coverage (16/16 instructions)

## Common Commands

```bash
# Run tests with coverage
yarn test-coverage

# Run specific test file with coverage
yarn test-litesvm -- tests/swap.test.ts

# Run with grep pattern
yarn test-litesvm -- --grep "swap"
```

## Notes

- Coverage reports show **after each test file runs**
- Each test file displays its own coverage report
- Total coverage accumulates across all test files
- Use `trackInstruction()` in every test that exercises an instruction
- Include test names for better debugging and tracking

## Resources

- [`tests/utils/instruction-coverage.ts`](tests/utils/instruction-coverage.ts) - Implementation
- [`tests/basic.test.ts`](tests/basic.test.ts) - Example usage
- [`tests/futarchy.test.ts`](tests/futarchy.test.ts) - Example usage
- [`tests/README.md`](tests/README.md) - General testing guide

---

**Last Updated**: October 2024  
**Tool**: Custom instruction tracker  
**Coverage Type**: Instruction coverage  
**Supported Instructions**: 16 Omnipair program instructions
