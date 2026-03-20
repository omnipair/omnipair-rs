/**
 * Instruction Coverage Tracking for LiteSVM Tests
 * Tracks which program instructions are tested
 */

const testedInstructions = new Set<string>();
const instructionDetails = new Map<string, { count: number; tests: string[] }>();

// All Omnipair program instructions
const ALL_INSTRUCTIONS = [
  "viewPairData",
  "viewUserPositionData",
  "initFutarchyAuthority",
  "updateFutarchyAuthority",
  "claimProtocolFees",
  "distributeTokens",
  "initialize",
  "addLiquidity",
  "removeLiquidity",
  "swap",
  "addCollateral",
  "removeCollateral",
  "borrow",
  "repay",
  "liquidate",
  "flashloan"
];

/**
 * Track that an instruction was tested
 * @param instructionName Name of the instruction tested
 * @param testName Name of the test that used it
 */
export function trackInstruction(instructionName: string, testName?: string) {
  testedInstructions.add(instructionName);
  
  const detail = instructionDetails.get(instructionName) || { count: 0, tests: [] };
  detail.count++;
  if (testName && !detail.tests.includes(testName)) {
    detail.tests.push(testName);
  }
  instructionDetails.set(instructionName, detail);
  
  console.log(`  ✓ Tested: ${instructionName}`);
}

/**
 * Get the coverage report
 */
export function getCoverageReport() {
  const covered = testedInstructions.size;
  const total = ALL_INSTRUCTIONS.length;
  const percentage = ((covered / total) * 100).toFixed(2);
  
  console.log("\n" + "═".repeat(70));
  console.log("📊 INSTRUCTION COVERAGE REPORT");
  console.log("═".repeat(70));
  
  console.log(`\n✅ Covered Instructions: ${covered}/${total} (${percentage}%)\n`);
  
  testedInstructions.forEach(ix => {
    const detail = instructionDetails.get(ix);
    const testCount = detail?.tests.length || 0;
    console.log(`  ✓ ${ix.padEnd(25)} [${testCount} test(s)]`);
    if (detail?.tests.length) {
      detail.tests.forEach(test => {
        console.log(`    └─ ${test}`);
      });
    }
  });
  
  const untested = ALL_INSTRUCTIONS.filter(ix => !testedInstructions.has(ix));
  
  if (untested.length > 0) {
    console.log(`\n❌ Untested Instructions: ${untested.length}/${total}\n`);
    untested.forEach(ix => {
      console.log(`  ✗ ${ix}`);
    });
  }
  
  console.log("\n" + "═".repeat(70));
  console.log(`Coverage: ${percentage}% | Tests: ${covered}/${total}`);
  console.log("═".repeat(70) + "\n");
  
  return {
    covered,
    total,
    percentage: parseFloat(percentage),
    testedInstructions: Array.from(testedInstructions),
    untestedInstructions: untested
  };
}

/**
 * Reset coverage tracking (for new test suite)
 */
export function resetCoverage() {
  testedInstructions.clear();
  instructionDetails.clear();
}

/**
 * Get current coverage as object
 */
export function getCoverageData() {
  return {
    covered: testedInstructions.size,
    total: ALL_INSTRUCTIONS.length,
    percentage: ((testedInstructions.size / ALL_INSTRUCTIONS.length) * 100).toFixed(2),
    testedInstructions: Array.from(testedInstructions),
    untestedInstructions: ALL_INSTRUCTIONS.filter(ix => !testedInstructions.has(ix))
  };
}
