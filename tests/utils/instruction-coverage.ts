/**
 * Instruction smoke coverage tracking for LiteSVM tests.
 * Tracks which program instructions are exercised at least once.
 */

type ProgramGeneration = "v1" | "v2";
type InstructionId = `${ProgramGeneration}:${string}`;

const testedInstructions = new Set<InstructionId>();
const instructionDetails = new Map<InstructionId, { count: number; tests: string[] }>();
let lastPrintedReportSignature: string | undefined;

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
  "initFutarchyAuthority",
  "updateFutarchyAuthority",
  "updateProtocolRevenue",
  "updateRevenueRecipients",
  "setGlobalReduceOnly",
  "initialize",
  "initializeLpMetadata",
  "updateConfig",
  "setReduceOnly",
  "addLiquidity",
  "removeLiquidity",
  "setYieldRecipient",
  "claimYield",
  "swap",
  "depositCollateral",
  "withdrawCollateral",
  "borrow",
  "repay",
  "openLiquidationAuction",
  "settleLiquidationAuction",
  "openHedge",
  "closeHedge",
];

const ALL_INSTRUCTIONS = [
  ...V1_INSTRUCTIONS.map((name) => instructionId("v1", name)),
  ...V2_INSTRUCTIONS.map((name) => instructionId("v2", name)),
];

const INSTRUCTIONS_BY_GENERATION: Record<ProgramGeneration, string[]> = {
  v1: V1_INSTRUCTIONS,
  v2: V2_INSTRUCTIONS,
};

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

function generationInstructions(generation: ProgramGeneration): InstructionId[] {
  return INSTRUCTIONS_BY_GENERATION[generation].map((name) =>
    instructionId(generation, name)
  );
}

function coverageDataFor(instructions: InstructionId[]) {
  const coveredInstructions = instructions.filter((ix) => testedInstructions.has(ix));
  const untestedInstructions = instructions.filter((ix) => !testedInstructions.has(ix));
  const covered = coveredInstructions.length;
  const total = instructions.length;
  const percentage = total === 0 ? "100.00" : ((covered / total) * 100).toFixed(2);

  return {
    covered,
    total,
    percentage,
    testedInstructions: coveredInstructions,
    untestedInstructions,
  };
}

function reportSignature(): string {
  return Array.from(testedInstructions).sort().join("|");
}

function printCoverageSection(title: string, instructions: InstructionId[]) {
  const data = coverageDataFor(instructions);

  console.log(`\n${title}`);
  console.log(
    `Instructions Exercised: ${data.covered}/${data.total} (${data.percentage}%)\n`
  );

  data.testedInstructions.forEach((ix) => {
    const detail = instructionDetails.get(ix);
    const testCount = detail?.tests.length || 0;
    console.log(`  ✓ ${instructionLabel(ix).padEnd(28)} [${testCount} test(s)]`);
    if (detail?.tests.length) {
      detail.tests.forEach((test) => {
        console.log(`    └─ ${test}`);
      });
    }
  });

  if (data.untestedInstructions.length > 0) {
    console.log(`\nUnexercised Instructions: ${data.untestedInstructions.length}/${data.total}\n`);
    data.untestedInstructions.forEach((ix) => {
      console.log(`  ✗ ${instructionLabel(ix)}`);
    });
  }
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
  const aggregate = coverageDataFor(ALL_INSTRUCTIONS);
  const signature = reportSignature();

  if (signature === lastPrintedReportSignature) {
    return {
      covered: aggregate.covered,
      total: aggregate.total,
      percentage: parseFloat(aggregate.percentage),
      testedInstructions: aggregate.testedInstructions.map(instructionLabel),
      untestedInstructions: aggregate.untestedInstructions.map(instructionLabel),
      byGeneration: {
        v1: getCoverageData("v1"),
        v2: getCoverageData("v2"),
      },
    };
  }
  lastPrintedReportSignature = signature;
  
  console.log("\n" + "═".repeat(70));
  console.log("📊 INSTRUCTION SMOKE COVERAGE REPORT");
  console.log("═".repeat(70));
  console.log(
    "This tracks whether each instruction is exercised by at least one LiteSVM test."
  );
  console.log(
    "It is not statement, branch, invariant, or full behavioral coverage."
  );

  printCoverageSection("V2 Instruction Smoke Coverage", generationInstructions("v2"));
  printCoverageSection("V1 Legacy Instruction Smoke Coverage", generationInstructions("v1"));
  
  console.log("\n" + "═".repeat(70));
  console.log(
    `Aggregate Smoke Coverage: ${aggregate.percentage}% | Instructions Exercised: ${aggregate.covered}/${aggregate.total}`
  );
  console.log("═".repeat(70) + "\n");
  
  return {
    covered: aggregate.covered,
    total: aggregate.total,
    percentage: parseFloat(aggregate.percentage),
    testedInstructions: aggregate.testedInstructions.map(instructionLabel),
    untestedInstructions: aggregate.untestedInstructions.map(instructionLabel),
    byGeneration: {
      v1: getCoverageData("v1"),
      v2: getCoverageData("v2"),
    },
  };
}

/**
 * Reset coverage tracking (for new test suite)
 */
export function resetCoverage() {
  testedInstructions.clear();
  instructionDetails.clear();
  lastPrintedReportSignature = undefined;
}

/**
 * Get current coverage as object
 */
export function getCoverageData(generation?: ProgramGeneration) {
  const instructions = generation ? generationInstructions(generation) : ALL_INSTRUCTIONS;
  const data = coverageDataFor(instructions);

  return {
    covered: data.covered,
    total: data.total,
    percentage: data.percentage,
    testedInstructions: data.testedInstructions.map(instructionLabel),
    untestedInstructions: data.untestedInstructions.map(instructionLabel),
  };
}
