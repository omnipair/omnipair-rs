/**
 * Instruction Coverage Tracking for LiteSVM Tests
 * Tracks which program instructions are tested
 */

type ProgramGeneration = "v1" | "v2";
type InstructionId = `${ProgramGeneration}:${string}`;

const testedInstructions = new Set<InstructionId>();
const instructionDetails = new Map<InstructionId, { count: number; tests: string[] }>();

const V1_INSTRUCTIONS = [
  "viewPairData",
  "viewUserPositionData",
  "initFutarchyAuthority",
  "updateFutarchyAuthority",
  "updateProtocolRevenue",
  "updateRevenueRecipients",
  "claimProtocolFees",
  "setGlobalReduceOnly",
  "setPairReduceOnly",
  "setPairRateModel",
  "createRateModel",
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

const V2_INSTRUCTIONS = [
  "initialize",
  "updateConfig",
  "setReduceOnly",
  "addLiquidity",
  "removeLiquidity",
  "stake",
  "unstake",
  "claimFees",
  "claimMarketFees",
  "swap",
  "depositCollateral",
  "withdrawCollateral",
  "borrow",
  "repay",
  "depositInsurance",
  "liquidate",
  "openHedge",
  "closeHedge",
  "claimHedgeFees"
];

const ALL_INSTRUCTIONS = [
  ...V1_INSTRUCTIONS.map((name) => instructionId("v1", name)),
  ...V2_INSTRUCTIONS.map((name) => instructionId("v2", name)),
];

function instructionId(generation: ProgramGeneration, instructionName: string): InstructionId {
  return `${generation}:${instructionName}`;
}

function instructionLabel(id: InstructionId): string {
  const [generation, instructionName] = id.split(":");
  return `${generation}.${instructionName}`;
}

function track(generation: ProgramGeneration, instructionName: string, testName?: string) {
  const id = instructionId(generation, instructionName);
  testedInstructions.add(id);

  const detail = instructionDetails.get(id) || { count: 0, tests: [] };
  detail.count++;
  if (testName && !detail.tests.includes(testName)) {
    detail.tests.push(testName);
  }
  instructionDetails.set(id, detail);

  console.log(`  ✓ Tested: ${instructionLabel(id)}`);
}

/**
 * Track that an instruction was tested
 * @param instructionName Name of the instruction tested
 * @param testName Name of the test that used it
 */
export function trackInstruction(instructionName: string, testName?: string) {
  track("v1", instructionName, testName);
}

/**
 * Track that a standalone v2 instruction was tested.
 * Keeps clean v2 names like swap/borrow separate from legacy v1 names.
 */
export function trackV2Instruction(instructionName: string, testName?: string) {
  track("v2", instructionName, testName);
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
    console.log(`  ✓ ${instructionLabel(ix).padEnd(28)} [${testCount} test(s)]`);
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
      console.log(`  ✗ ${instructionLabel(ix)}`);
    });
  }
  
  console.log("\n" + "═".repeat(70));
  console.log(`Coverage: ${percentage}% | Tests: ${covered}/${total}`);
  console.log("═".repeat(70) + "\n");
  
  return {
    covered,
    total,
    percentage: parseFloat(percentage),
    testedInstructions: Array.from(testedInstructions).map(instructionLabel),
    untestedInstructions: untested.map(instructionLabel)
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
    testedInstructions: Array.from(testedInstructions).map(instructionLabel),
    untestedInstructions: ALL_INSTRUCTIONS
      .filter(ix => !testedInstructions.has(ix))
      .map(instructionLabel)
  };
}
