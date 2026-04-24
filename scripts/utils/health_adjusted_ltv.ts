import { AnchorProvider, Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import idl from "../../target/idl/omnipair.json" with { type: "json" };
import type { Omnipair } from "../../target/types/omnipair";

const NAD = 1_000_000_000n;
const BPS_DENOMINATOR = 10_000n;
const LTV_BUFFER_BPS = 500n;
const MAX_COLLATERAL_FACTOR_BPS = 8_500n;
const DIRECTIONAL_EMA_HALF_LIFE_MS = 3_000n;
const TARGET_MS_PER_SLOT = 400n;
const TAYLOR_TERMS = 5n;
const NATURAL_LOG_OF_TWO_NAD = 693_147_180n;
const MILLISECONDS_PER_YEAR = 31_536_000_000n;
const MIN_LIQUIDITY = 1_000n;

export type CollateralSide = "token0" | "token1";

export type ValidationCase = {
  label: string;
  pairAddress: string;
  collateralSide: CollateralSide;
};

type PairSnapshot = {
  address: PublicKey;
  token0: PublicKey;
  token1: PublicKey;
  rateModel: PublicKey;
  fixedCfBps: number | null;
  reserve0: bigint;
  reserve1: bigint;
  cashReserve0: bigint;
  cashReserve1: bigint;
  totalDebt0: bigint;
  totalDebt1: bigint;
  totalCollateral0: bigint;
  totalCollateral1: bigint;
  token0Decimals: number;
  token1Decimals: number;
  halfLife: bigint;
  lastUpdate: bigint;
  lastRate0: bigint;
  lastRate1: bigint;
  lastPrice0Symmetric: bigint;
  lastPrice0Directional: bigint;
  lastPrice1Symmetric: bigint;
  lastPrice1Directional: bigint;
};

type RateModelSnapshot = {
  expRate: bigint;
  targetUtilStart: bigint;
  targetUtilEnd: bigint;
  minRate: bigint;
  maxRate: bigint;
};

type FutarchyAuthoritySnapshot = {
  interestBps: bigint;
};

export type BorrowLimitQuote = {
  borrowLimit: bigint;
  maxCfBps: number;
  liquidationCfBps: number;
};

export type ValidationResult = {
  label: string;
  pairAddress: string;
  collateralSide: CollateralSide;
  collateralAmount: bigint;
  collateralMint: PublicKey;
  debtMint: PublicKey;
  collateralDecimals: number;
  debtDecimals: number;
  legacy: BorrowLimitQuote;
  proposedOffchain: BorrowLimitQuote;
  proposedOnchain: BorrowLimitQuote;
};

function toBigInt(value: unknown): bigint {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  if (typeof value === "string") {
    return BigInt(value);
  }
  if (value && typeof value === "object" && "toString" in value) {
    return BigInt((value as { toString(): string }).toString());
  }
  throw new Error(`Unsupported bigint conversion for value: ${String(value)}`);
}

function toOptionalNumber(value: unknown): number | null {
  if (value == null) {
    return null;
  }
  if (typeof value === "number") {
    return value;
  }
  if (typeof value === "object") {
    const candidate = value as { some?: unknown };
    if (candidate.some != null) {
      return Number(candidate.some);
    }
  }
  return Number(value);
}

function ceilDiv(a: bigint, b: bigint): bigint {
  if (b === 0n) {
    throw new Error("division by zero");
  }
  return (a + b - 1n) / b;
}

function sqrtBigInt(value: bigint): bigint {
  if (value < 0n) {
    throw new Error("sqrt of negative bigint");
  }
  if (value > 3n) {
    let z = value;
    let x = value / 2n + 1n;
    while (x < z) {
      z = x;
      x = (value / x + x) / 2n;
    }
    return z;
  }
  return value === 0n ? 0n : 1n;
}

function slotsToMs(startSlot: bigint, endSlot: bigint): bigint {
  if (endSlot < startSlot) {
    return 0n;
  }
  return (endSlot - startSlot) * TARGET_MS_PER_SLOT;
}

function taylorExp(x: bigint, scale: bigint, precision: bigint): bigint {
  const isNegative = x < 0n;
  const absX = isNegative ? -x : x;
  const n = 10n;
  const reducedX = absX / n;

  let term = scale;
  let sum = scale;
  for (let i = 1n; i <= precision; i += 1n) {
    term = (term * reducedX) / (i * scale);
    sum += term;
  }

  let result = scale;
  for (let i = 0n; i < n; i += 1n) {
    result = (result * sum) / scale;
  }

  if (isNegative) {
    result = (scale * scale) / result;
  }

  return result;
}

function computeEma(
  lastEma: bigint,
  lastUpdate: bigint,
  input: bigint,
  halfLife: bigint,
  currentSlot: bigint,
): bigint {
  const dt = slotsToMs(lastUpdate, currentSlot);
  if (dt > 0n && halfLife > 0n) {
    const x = (dt * NATURAL_LOG_OF_TWO_NAD) / halfLife;
    const alpha = taylorExp(-x, NAD, TAYLOR_TERMS);
    return (input * (NAD - alpha) + lastEma * alpha) / NAD;
  }
  return lastEma;
}

function lnNad(xNad: bigint): bigint {
  if (xNad <= 0n) {
    throw new Error("ln_nad requires x > 0");
  }

  let z = xNad;
  let k = 0n;
  while (z < NAD / 2n) {
    z *= 2n;
    k -= 1n;
  }
  while (z >= NAD * 2n) {
    z /= 2n;
    k += 1n;
  }

  const v = ((z - NAD) * NAD) / (z + NAD);
  const v2 = (v * v) / NAD;
  const v3 = (v2 * v) / NAD;
  const v5 = (v3 * v2) / NAD;
  const v7 = (v5 * v2) / NAD;
  const v9 = (v7 * v2) / NAD;
  const series = v + v3 / 3n + v5 / 5n + v7 / 7n + v9 / 9n;

  return 2n * series + k * NATURAL_LOG_OF_TWO_NAD;
}

function timeToReachClosedForm(
  r0: bigint,
  target: bigint,
  expRate: bigint,
  up: boolean,
): bigint {
  if (up) {
    if (target <= r0) {
      return 0n;
    }
    const ratioNad = (target * NAD) / (r0 === 0n ? 1n : r0);
    const t = lnNad(ratioNad) / expRate;
    return t <= 0n ? 0n : t;
  }

  if (r0 <= target) {
    return 0n;
  }
  const ratioNad = (r0 * NAD) / (target === 0n ? 1n : target);
  const t = lnNad(ratioNad) / expRate;
  return t <= 0n ? 0n : t;
}

function calculateRate(
  rateModel: RateModelSnapshot,
  lastRate: bigint,
  timeElapsed: bigint,
  lastUtil: bigint,
): { currentRate: bigint; integral: bigint } {
  if (timeElapsed === 0n) {
    return { currentRate: lastRate, integral: 0n };
  }

  const expRate = rateModel.expRate;
  const x = expRate * timeElapsed;
  const gd = taylorExp(-x, NAD, TAYLOR_TERMS);
  const minNad = rateModel.minRate;
  const maxNad = rateModel.maxRate;
  const hasMaxCap = maxNad > 0n;
  const last = lastRate > minNad ? lastRate : minNad;

  if (lastUtil > rateModel.targetUtilEnd) {
    const currUnclamped = (last * NAD) / (gd === 0n ? 1n : gd);
    let current = currUnclamped;

    if (hasMaxCap && currUnclamped > maxNad) {
      if (last >= maxNad) {
        return {
          currentRate: maxNad,
          integral: ceilDiv(maxNad * timeElapsed, MILLISECONDS_PER_YEAR),
        };
      }

      const tToMax = timeToReachClosedForm(last, maxNad, expRate, true);
      const boundedT = tToMax < timeElapsed ? tToMax : timeElapsed;
      const expPart = ceilDiv((maxNad - last) * NAD, expRate);
      const flatPart = maxNad * (timeElapsed - boundedT);
      return {
        currentRate: maxNad,
        integral: ceilDiv(expPart + flatPart, MILLISECONDS_PER_YEAR),
      };
    }

    const integralPre = ((current - last) * NAD) / expRate;
    return {
      currentRate: current,
      integral: ceilDiv(integralPre, MILLISECONDS_PER_YEAR),
    };
  }

  if (lastUtil < rateModel.targetUtilStart) {
    const currentUnclamped = (last * gd) / NAD;
    if (currentUnclamped >= minNad) {
      const integralPre = ((last - currentUnclamped) * NAD) / expRate;
      return {
        currentRate: currentUnclamped,
        integral: ceilDiv(integralPre, MILLISECONDS_PER_YEAR),
      };
    }

    if (last <= minNad) {
      return {
        currentRate: minNad,
        integral: ceilDiv(minNad * timeElapsed, MILLISECONDS_PER_YEAR),
      };
    }

    const tToMin = timeToReachClosedForm(last, minNad, expRate, false);
    const boundedT = tToMin < timeElapsed ? tToMin : timeElapsed;
    const expPart = ceilDiv((last - minNad) * NAD, expRate);
    const flatPart = minNad * (timeElapsed - boundedT);
    return {
      currentRate: minNad,
      integral: ceilDiv(expPart + flatPart, MILLISECONDS_PER_YEAR),
    };
  }

  return {
    currentRate: last,
    integral: ceilDiv(last * timeElapsed, MILLISECONDS_PER_YEAR),
  };
}

function normalizePair(address: PublicKey, pairAccount: any): PairSnapshot {
  return {
    address,
    token0: pairAccount.token0 as PublicKey,
    token1: pairAccount.token1 as PublicKey,
    rateModel: pairAccount.rateModel as PublicKey,
    fixedCfBps: toOptionalNumber(pairAccount.fixedCfBps),
    reserve0: toBigInt(pairAccount.reserve0),
    reserve1: toBigInt(pairAccount.reserve1),
    cashReserve0: toBigInt(pairAccount.cashReserve0),
    cashReserve1: toBigInt(pairAccount.cashReserve1),
    totalDebt0: toBigInt(pairAccount.totalDebt0),
    totalDebt1: toBigInt(pairAccount.totalDebt1),
    totalCollateral0: toBigInt(pairAccount.totalCollateral0),
    totalCollateral1: toBigInt(pairAccount.totalCollateral1),
    token0Decimals: Number(pairAccount.token0Decimals),
    token1Decimals: Number(pairAccount.token1Decimals),
    halfLife: toBigInt(pairAccount.halfLife),
    lastUpdate: toBigInt(pairAccount.lastUpdate),
    lastRate0: toBigInt(pairAccount.lastRate0),
    lastRate1: toBigInt(pairAccount.lastRate1),
    lastPrice0Symmetric: toBigInt(pairAccount.lastPrice0Ema.symmetric),
    lastPrice0Directional: toBigInt(pairAccount.lastPrice0Ema.directional),
    lastPrice1Symmetric: toBigInt(pairAccount.lastPrice1Ema.symmetric),
    lastPrice1Directional: toBigInt(pairAccount.lastPrice1Ema.directional),
  };
}

function normalizeRateModel(rateModel: any): RateModelSnapshot {
  return {
    expRate: toBigInt(rateModel.expRate),
    targetUtilStart: toBigInt(rateModel.targetUtilStart),
    targetUtilEnd: toBigInt(rateModel.targetUtilEnd),
    minRate: toBigInt(rateModel.minRate),
    maxRate: toBigInt(rateModel.maxRate),
  };
}

function normalizeFutarchyAuthority(authority: any): FutarchyAuthoritySnapshot {
  return {
    interestBps: toBigInt(authority.revenueShare.interestBps),
  };
}

function spotPriceNad(collateralReserve: bigint, debtReserve: bigint): bigint {
  if (collateralReserve === 0n) {
    return 0n;
  }
  return (debtReserve * NAD) / collateralReserve;
}

function updatePairForView(
  pair: PairSnapshot,
  rateModel: RateModelSnapshot,
  futarchyAuthority: FutarchyAuthoritySnapshot,
  currentSlot: bigint,
): PairSnapshot {
  const updated: PairSnapshot = { ...pair };
  const spotPrice0 = spotPriceNad(updated.reserve0, updated.reserve1);
  const spotPrice1 = spotPriceNad(updated.reserve1, updated.reserve0);

  updated.lastPrice0Directional =
    updated.lastPrice0Directional < spotPrice0 ? updated.lastPrice0Directional : spotPrice0;
  updated.lastPrice1Directional =
    updated.lastPrice1Directional < spotPrice1 ? updated.lastPrice1Directional : spotPrice1;

  if (currentSlot > updated.lastUpdate) {
    const timeElapsed = slotsToMs(updated.lastUpdate, currentSlot);
    if (timeElapsed > 0n) {
      updated.lastPrice0Symmetric = computeEma(
        updated.lastPrice0Symmetric,
        updated.lastUpdate,
        spotPrice0,
        updated.halfLife,
        currentSlot,
      );
      updated.lastPrice1Symmetric = computeEma(
        updated.lastPrice1Symmetric,
        updated.lastUpdate,
        spotPrice1,
        updated.halfLife,
        currentSlot,
      );

      const newDirectional0 = computeEma(
        updated.lastPrice0Directional,
        updated.lastUpdate,
        spotPrice0,
        DIRECTIONAL_EMA_HALF_LIFE_MS,
        currentSlot,
      );
      updated.lastPrice0Directional =
        spotPrice0 < newDirectional0 ? spotPrice0 : newDirectional0;

      const newDirectional1 = computeEma(
        updated.lastPrice1Directional,
        updated.lastUpdate,
        spotPrice1,
        DIRECTIONAL_EMA_HALF_LIFE_MS,
        currentSlot,
      );
      updated.lastPrice1Directional =
        spotPrice1 < newDirectional1 ? spotPrice1 : newDirectional1;

      const util0 = updated.reserve0 === 0n ? 0n : (updated.totalDebt0 * NAD) / updated.reserve0;
      const util1 = updated.reserve1 === 0n ? 0n : (updated.totalDebt1 * NAD) / updated.reserve1;

      const rate0 = calculateRate(rateModel, updated.lastRate0, timeElapsed, util0);
      const rate1 = calculateRate(rateModel, updated.lastRate1, timeElapsed, util1);

      updated.lastRate0 = rate0.currentRate;
      updated.lastRate1 = rate1.currentRate;

      const totalInterest0 = ceilDiv(updated.totalDebt0 * rate0.integral, NAD);
      const totalInterest1 = ceilDiv(updated.totalDebt1 * rate1.integral, NAD);

      const protocolFee0 =
        (totalInterest0 * futarchyAuthority.interestBps) / BPS_DENOMINATOR;
      const protocolFee1 =
        (totalInterest1 * futarchyAuthority.interestBps) / BPS_DENOMINATOR;

      const totalBorrowerCost0 = totalInterest0 + protocolFee0;
      const totalBorrowerCost1 = totalInterest1 + protocolFee1;
      updated.totalDebt0 += totalBorrowerCost0;
      updated.totalDebt1 += totalBorrowerCost1;

      const cashCoveredFee0 =
        protocolFee0 < updated.cashReserve0 ? protocolFee0 : updated.cashReserve0;
      const cashCoveredFee1 =
        protocolFee1 < updated.cashReserve1 ? protocolFee1 : updated.cashReserve1;

      updated.reserve0 += totalInterest0 + (protocolFee0 - cashCoveredFee0);
      updated.reserve1 += totalInterest1 + (protocolFee1 - cashCoveredFee1);
      updated.cashReserve0 -= cashCoveredFee0;
      updated.cashReserve1 -= cashCoveredFee1;
      updated.lastUpdate = currentSlot;
    }
  }

  return updated;
}

function constructVirtualReservesAtPessimisticPrice(
  collateralSpotReserve: bigint,
  debtSpotReserve: bigint,
  collateralEmaPriceNad: bigint,
  collateralDirectionalEmaPriceNad: bigint,
): [bigint, bigint] {
  if (collateralSpotReserve < MIN_LIQUIDITY || debtSpotReserve < MIN_LIQUIDITY) {
    return [0n, 0n];
  }

  const pessimisticPrice =
    collateralDirectionalEmaPriceNad < collateralEmaPriceNad
      ? collateralDirectionalEmaPriceNad
      : collateralEmaPriceNad;

  if (pessimisticPrice === 0n) {
    return [collateralSpotReserve, debtSpotReserve];
  }

  const spotK = collateralSpotReserve * debtSpotReserve;
  const collateralVirt = sqrtBigInt((spotK * NAD) / pessimisticPrice);
  const debtVirt = sqrtBigInt((spotK * pessimisticPrice) / NAD);
  return [collateralVirt, debtVirt];
}

function cpAmountOut(reserveIn: bigint, reserveOut: bigint, amountIn: bigint): bigint {
  const denominator = reserveIn + amountIn;
  if (denominator === 0n) {
    throw new Error("cp amount out denominator is zero");
  }
  return (amountIn * reserveOut) / denominator;
}

function cpAmountIn(reserveIn: bigint, reserveOut: bigint, amountOut: bigint): bigint {
  const denominator = reserveOut - amountOut;
  if (denominator <= 0n) {
    throw new Error("cp amount in denominator is non-positive");
  }
  return ceilDiv(amountOut * reserveIn, denominator);
}

function asNumber(value: bigint, label: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${label} exceeds MAX_SAFE_INTEGER`);
  }
  return Number(value);
}

function quoteBorrowLimit(
  pair: PairSnapshot,
  collateralSide: CollateralSide,
  collateralAmount: bigint,
  useHealthAdjustedProxy: boolean,
): BorrowLimitQuote {
  const collateralEmaPriceNad =
    collateralSide === "token0" ? pair.lastPrice0Symmetric : pair.lastPrice1Symmetric;
  const collateralDirectionalEmaPriceNad =
    collateralSide === "token0" ? pair.lastPrice0Directional : pair.lastPrice1Directional;
  const collateralReserve = collateralSide === "token0" ? pair.reserve0 : pair.reserve1;
  const debtReserve = collateralSide === "token0" ? pair.reserve1 : pair.reserve0;
  const totalDebt = collateralSide === "token0" ? pair.totalDebt1 : pair.totalDebt0;
  const totalCollateralForSide =
    collateralSide === "token0" ? pair.totalCollateral0 : pair.totalCollateral1;

  if (
    collateralAmount === 0n ||
    collateralEmaPriceNad === 0n ||
    collateralDirectionalEmaPriceNad === 0n
  ) {
    return { borrowLimit: 0n, maxCfBps: 0, liquidationCfBps: 0 };
  }

  const [collateralVirt, debtVirt] = constructVirtualReservesAtPessimisticPrice(
    collateralReserve,
    debtReserve,
    collateralEmaPriceNad,
    collateralDirectionalEmaPriceNad,
  );

  const collateralValueWithImpact = cpAmountOut(collateralVirt, debtVirt, collateralAmount);

  let baseCfBps: bigint;
  const fixedCfBps = pair.fixedCfBps;
  if (fixedCfBps != null) {
    baseCfBps = BigInt(fixedCfBps);
  } else {
    if (debtReserve === 0n) {
      return { borrowLimit: 0n, maxCfBps: 0, liquidationCfBps: 0 };
    }

    let effectiveTotalDebt = totalDebt;
    if (useHealthAdjustedProxy && totalDebt > 0n) {
      const poolCollateralValueWithImpact = cpAmountOut(
        collateralVirt,
        debtVirt,
        totalCollateralForSide,
      );
      const rawEffective = ceilDiv(totalDebt * totalDebt, poolCollateralValueWithImpact > 0n ? poolCollateralValueWithImpact : 1n);
      effectiveTotalDebt = rawEffective < totalDebt ? rawEffective : totalDebt;
    }

    const utilizedCollateral = cpAmountIn(collateralVirt, debtVirt, effectiveTotalDebt);
    const maxAllowedTotalDebt = cpAmountOut(
      collateralVirt,
      debtVirt,
      utilizedCollateral + collateralAmount,
    );
    const userMaxDebt =
      maxAllowedTotalDebt > effectiveTotalDebt ? maxAllowedTotalDebt - effectiveTotalDebt : 0n;

    baseCfBps =
      collateralValueWithImpact === 0n
        ? 0n
        : (userMaxDebt * BPS_DENOMINATOR) / collateralValueWithImpact;
  }

  let liquidationCfBps: bigint;
  if (fixedCfBps != null) {
    const shrunk =
      (collateralDirectionalEmaPriceNad * baseCfBps) /
      (collateralEmaPriceNad === 0n ? 1n : collateralEmaPriceNad);
    const capped = baseCfBps < shrunk ? baseCfBps : shrunk;
    liquidationCfBps = capped > 100n ? capped : 100n;
  } else {
    liquidationCfBps =
      baseCfBps < MAX_COLLATERAL_FACTOR_BPS ? baseCfBps : MAX_COLLATERAL_FACTOR_BPS;
  }

  const maxAllowedCfBps =
    (liquidationCfBps * (BPS_DENOMINATOR - LTV_BUFFER_BPS)) / BPS_DENOMINATOR;
  const borrowLimit = (collateralValueWithImpact * maxAllowedCfBps) / BPS_DENOMINATOR;

  return {
    borrowLimit,
    maxCfBps: asNumber(maxAllowedCfBps, "maxAllowedCfBps"),
    liquidationCfBps: asNumber(liquidationCfBps, "liquidationCfBps"),
  };
}

function parseBorrowLimitQuoteFromLogs(logs: string[]): BorrowLimitQuote {
  const line = logs.find((entry) => entry.includes("GetBorrowLimitAndCfBpsForCollateral"));
  if (!line) {
    throw new Error(`Borrow-limit log not found in simulation logs:\n${logs.join("\n")}`);
  }

  const match = line.match(
    /GetBorrowLimitAndCfBpsForCollateral:\s*\(U64\((\d+)\),\s*U16\((\d+)\),\s*U16\((\d+)\)\)/,
  );

  if (!match) {
    throw new Error(`Could not parse borrow-limit log line: ${line}`);
  }

  return {
    borrowLimit: BigInt(match[1]),
    maxCfBps: Number(match[2]),
    liquidationCfBps: Number(match[3]),
  };
}

export function formatBps(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`;
}

export function formatTokenAmount(rawAmount: bigint, decimals: number): string {
  const base = 10n ** BigInt(decimals);
  const whole = rawAmount / base;
  const fraction = rawAmount % base;
  const fractionString = fraction.toString().padStart(decimals, "0").replace(/0+$/, "");
  return fractionString.length > 0 ? `${whole}.${fractionString}` : whole.toString();
}

export function buildProgram(provider?: AnchorProvider): Program<Omnipair> {
  const resolvedProvider = provider ?? AnchorProvider.env();
  return new Program<Omnipair>(idl as Omnipair, resolvedProvider);
}

export function getFutarchyAuthorityPda(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("futarchy_authority")], programId)[0];
}

async function simulateOnchainBorrowLimit(
  program: Program<Omnipair>,
  pair: PairSnapshot,
  futarchyAuthority: PublicKey,
  collateralMint: PublicKey,
  collateralAmount: bigint,
): Promise<BorrowLimitQuote> {
  const simulation = await program.methods
    .viewPairData(
      { getBorrowLimitAndCfBpsForCollateral: {} },
      {
        amount: new BN(asNumber(collateralAmount, "collateralAmount")),
        tokenMint: collateralMint,
        debtAmount: null,
      },
    )
    .accountsPartial({
      pair: pair.address,
      rateModel: pair.rateModel,
      futarchyAuthority,
    })
    .simulate();

  const logs = (((simulation as any).raw ?? (simulation as any).logs ?? []) as string[]).map(String);
  return parseBorrowLimitQuoteFromLogs(logs);
}

export async function runValidationCase(
  program: Program<Omnipair>,
  validationCase: ValidationCase,
): Promise<ValidationResult> {
  const pairAddress = new PublicKey(validationCase.pairAddress);
  const pairAccount = await program.account.pair.fetch(pairAddress);
  const normalizedPair = normalizePair(pairAddress, pairAccount);
  const rateModelAccount = await program.account.rateModel.fetch(normalizedPair.rateModel);
  const futarchyAuthorityPda = getFutarchyAuthorityPda(program.programId);
  const futarchyAuthorityAccount =
    await program.account.futarchyAuthority.fetch(futarchyAuthorityPda);
  const currentSlot = BigInt(await program.provider.connection.getSlot("confirmed"));

  const updatedPair = updatePairForView(
    normalizedPair,
    normalizeRateModel(rateModelAccount),
    normalizeFutarchyAuthority(futarchyAuthorityAccount),
    currentSlot,
  );

  const collateralMint =
    validationCase.collateralSide === "token0" ? updatedPair.token0 : updatedPair.token1;
  const debtMint =
    validationCase.collateralSide === "token0" ? updatedPair.token1 : updatedPair.token0;
  const collateralReserve =
    validationCase.collateralSide === "token0" ? updatedPair.reserve0 : updatedPair.reserve1;
  const collateralAmount = collateralReserve / 100n;
  const debtDecimals =
    validationCase.collateralSide === "token0"
      ? updatedPair.token1Decimals
      : updatedPair.token0Decimals;
  const collateralDecimals =
    validationCase.collateralSide === "token0"
      ? updatedPair.token0Decimals
      : updatedPair.token1Decimals;

  const legacy = quoteBorrowLimit(
    updatedPair,
    validationCase.collateralSide,
    collateralAmount,
    false,
  );
  const proposedOffchain = quoteBorrowLimit(
    updatedPair,
    validationCase.collateralSide,
    collateralAmount,
    true,
  );
  const proposedOnchain = await simulateOnchainBorrowLimit(
    program,
    normalizedPair,
    futarchyAuthorityPda,
    collateralMint,
    collateralAmount,
  );

  return {
    label: validationCase.label,
    pairAddress: validationCase.pairAddress,
    collateralSide: validationCase.collateralSide,
    collateralAmount,
    collateralMint,
    debtMint,
    collateralDecimals,
    debtDecimals,
    legacy,
    proposedOffchain,
    proposedOnchain,
  };
}
