import { AnchorProvider } from "@coral-xyz/anchor";
import {
  buildProgram,
  formatBps,
  formatTokenAmount,
  runValidationCase,
  type CollateralSide,
  type ValidationCase,
} from "./utils/health_adjusted_ltv.ts";

type CliArgs = {
  pairAddress: string;
  collateralSide: CollateralSide;
  label: string;
};

function parseArgs(argv: string[]): CliArgs {
  const defaults: CliArgs = {
    pairAddress: "3cPJTS5kfD7414aTRPcyBrA55aSx8csCUPWsrS4mnFWV",
    collateralSide: "token0",
    label: "SOL / USDC",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--pair" && argv[index + 1]) {
      defaults.pairAddress = argv[index + 1];
      index += 1;
    } else if (value === "--side" && argv[index + 1]) {
      const side = argv[index + 1];
      if (side !== "token0" && side !== "token1") {
        throw new Error(`Unsupported side "${side}". Use token0 or token1.`);
      }
      defaults.collateralSide = side;
      index += 1;
    } else if (value === "--label" && argv[index + 1]) {
      defaults.label = argv[index + 1];
      index += 1;
    }
  }

  return defaults;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const provider = AnchorProvider.env();
  const program = buildProgram(provider);

  const validationCase: ValidationCase = {
    label: args.label,
    pairAddress: args.pairAddress,
    collateralSide: args.collateralSide,
  };

  const result = await runValidationCase(program, validationCase);
  console.log(`Health-Adjusted Dynamic LTV Validation`);
  console.log(`RPC: ${provider.connection.rpcEndpoint}`);
  console.log(`Pair: ${result.label} (${result.pairAddress})`);
  console.log(`Collateral side: ${result.collateralSide}`);
  console.log(
    `Collateral amount (1% reserve): ${formatTokenAmount(result.collateralAmount, result.collateralDecimals)} collateral tokens`,
  );
  console.log("");
  console.log(`Legacy max LTV: ${formatBps(result.legacy.maxCfBps)}`);
  console.log(`Legacy liquidation CF: ${formatBps(result.legacy.liquidationCfBps)}`);
  console.log(`Legacy borrow limit: ${formatTokenAmount(result.legacy.borrowLimit, result.debtDecimals)} debt tokens`);
  console.log("");
  console.log(`Proposed max LTV (off-chain): ${formatBps(result.proposedOffchain.maxCfBps)}`);
  console.log(`Proposed liquidation CF (off-chain): ${formatBps(result.proposedOffchain.liquidationCfBps)}`);
  console.log(`Proposed borrow limit (off-chain): ${formatTokenAmount(result.proposedOffchain.borrowLimit, result.debtDecimals)} debt tokens`);
  console.log("");
  console.log(`Proposed max LTV (on-chain view): ${formatBps(result.proposedOnchain.maxCfBps)}`);
  console.log(`Proposed liquidation CF (on-chain view): ${formatBps(result.proposedOnchain.liquidationCfBps)}`);
  console.log(`Proposed borrow limit (on-chain view): ${formatTokenAmount(result.proposedOnchain.borrowLimit, result.debtDecimals)} debt tokens`);
  console.log("");
  const deltaBps = result.proposedOnchain.maxCfBps - result.legacy.maxCfBps;
  const deltaPct = (deltaBps / 100).toFixed(2);
  console.log(`Delta max LTV (legacy -> on-chain): ${deltaPct}%`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
