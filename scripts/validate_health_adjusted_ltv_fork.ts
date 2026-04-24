import { AnchorProvider } from "@coral-xyz/anchor";
import {
  buildProgram,
  formatBps,
  formatTokenAmount,
  runValidationCase,
  type ValidationCase,
} from "./utils/health_adjusted_ltv.ts";

const FORK_CASES: ValidationCase[] = [
  {
    label: "LOYAL / USDC",
    pairAddress: "DYMhC9dXEpbRdwYSEPUjubj88udSgNYMpPQXENUh9bxE",
    collateralSide: "token0",
  },
  {
    label: "SOL / USDC",
    pairAddress: "3cPJTS5kfD7414aTRPcyBrA55aSx8csCUPWsrS4mnFWV",
    collateralSide: "token0",
  },
  {
    label: "USDC / USDT",
    pairAddress: "G7enNSGb5k264XRJCPuUXxAjQrWSEvu9qvWe4rg8ADAv",
    collateralSide: "token0",
  },
];

async function main() {
  const provider = AnchorProvider.env();
  const program = buildProgram(provider);
  const results = [];

  console.log(`Surfpool fork validation`);
  console.log(`RPC: ${provider.connection.rpcEndpoint}`);
  console.log("");

  for (const validationCase of FORK_CASES) {
    const result = await runValidationCase(program, validationCase);
    results.push(result);

    const deltaBps = result.proposedOnchain.maxCfBps - result.legacy.maxCfBps;
    console.log(`${result.label} (${result.pairAddress})`);
    console.log(`  side: ${result.collateralSide}`);
    console.log(
      `  collateral amount: ${formatTokenAmount(result.collateralAmount, result.collateralDecimals)}`,
    );
    console.log(
      `  legacy LTV: ${formatBps(result.legacy.maxCfBps)} | proposed LTV: ${formatBps(result.proposedOnchain.maxCfBps)} | delta: ${(deltaBps / 100).toFixed(2)}%`,
    );
    console.log(
      `  legacy liq CF: ${formatBps(result.legacy.liquidationCfBps)} | proposed liq CF: ${formatBps(result.proposedOnchain.liquidationCfBps)}`,
    );
    console.log(
      `  legacy borrow limit: ${formatTokenAmount(result.legacy.borrowLimit, result.debtDecimals)} | proposed borrow limit: ${formatTokenAmount(result.proposedOnchain.borrowLimit, result.debtDecimals)}`,
    );
    console.log("");
  }

  const [loyalUsdc, solUsdc, usdcUsdt] = results;

  if (loyalUsdc.proposedOnchain.maxCfBps <= loyalUsdc.legacy.maxCfBps) {
    throw new Error("Expected LOYAL / USDC proposed max LTV to be above legacy baseline");
  }
  if (solUsdc.proposedOnchain.maxCfBps <= solUsdc.legacy.maxCfBps) {
    throw new Error("Expected SOL / USDC proposed max LTV to be above legacy baseline");
  }

  const controlDelta = Math.abs(usdcUsdt.proposedOnchain.maxCfBps - usdcUsdt.legacy.maxCfBps);
  if (controlDelta > 5) {
    throw new Error(
      `Expected USDC / USDT control delta to stay within 5 bps, got ${controlDelta} bps`,
    );
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
