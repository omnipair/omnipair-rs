import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import http from "node:http";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import anchor from "@coral-xyz/anchor";
import BN from "bn.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  ExtensionType,
  NATIVE_MINT,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createInitializeMintInstruction,
  createInitializeTransferHookInstruction,
  getAccount,
  getAssociatedTokenAddressSync,
  getMint,
  getMintLen,
} from "@solana/spl-token";
import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";

const DEFAULT_PROGRAM_ID = "358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv";
const DEFAULT_META_MINT = "METAwkXcqyXKy1AtsSgJ8JiUHwGCafnZL38n3vYmeta";
const DEFAULT_USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const BPF_LOADER_UPGRADEABLE_ID = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
const TOKEN_METADATA_PROGRAM_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const NAD = 1_000_000_000n;

const SURFPOOL_RPC_URL = process.env.SURFPOOL_RPC_URL ?? "http://127.0.0.1:8899";
const PUBLIC_RPC_URL = normalizePublicUrl(
  process.env.PUBLIC_SURFPOOL_RPC_URL ?? process.env.SURFPOOL_RPC_PROXY_URL ?? SURFPOOL_RPC_URL
);
const PROGRAM_ID = new PublicKey(
  process.env.OMNIPAIR_V2_PROGRAM_ID ?? process.env.PROGRAM_ID_V2 ?? DEFAULT_PROGRAM_ID
);
const DEFAULT_SOL_FUNDING = Number(process.env.FORK_DEFAULT_SOL_FUNDING ?? "10");
const DEFAULT_TOKEN_FUNDING_UI = process.env.FORK_DEFAULT_TOKEN_FUNDING ?? "10000";
const MAX_SOL_FUNDING = Number(process.env.FORK_MAX_SOL_FUNDING ?? "100");
const MAX_TOKEN_FUNDING_UI = process.env.FORK_MAX_TOKEN_FUNDING ?? "1000000";
const DEFAULT_SEED_BASE_UI = process.env.OMNIPAIR_V2_BASE_LIQUIDITY ?? "100000";
const DEFAULT_SEED_QUOTE_UI = process.env.OMNIPAIR_V2_QUOTE_LIQUIDITY ?? "100000";
const ALLOW_PUBLIC_FUNDING = process.env.FORK_ALLOW_PUBLIC_FUNDING !== "false";

type MarketAsset = "base" | "quote";
type YieldTokenKind = "ylp" | "hlp";

type StoredMarket = {
  label: string;
  programId: string;
  market: string;
  paramsHash: string;
  baseMint: string;
  quoteMint: string;
  baseDecimals: number;
  quoteDecimals: number;
  baseTokenProgram: string;
  quoteTokenProgram: string;
  ylpMint: string;
  baseHlpMint: string;
  quoteHlpMint: string;
  ylpTokenMetadata: string;
  baseHlpTokenMetadata: string;
  quoteHlpTokenMetadata: string;
  baseReserveVault: string;
  quoteReserveVault: string;
  baseCollateralVault: string;
  quoteCollateralVault: string;
  baseInsuranceVault: string;
  quoteInsuranceVault: string;
  baseFeeVault: string;
  quoteFeeVault: string;
  baseInterestVault: string;
  quoteInterestVault: string;
  baseHlpYlpVault: string;
  quoteHlpYlpVault: string;
  eventAuthority: string;
  seededLiquidity: boolean;
  transferHookValidationAccounts: Record<string, string>;
};

type ForkState = {
  markets: Record<string, StoredMarket>;
};

let runtime:
  | {
      payer: Keypair;
      connection: Connection;
      provider: anchor.AnchorProvider;
      program: any;
      idl: anchor.Idl;
      accountCoder: anchor.BorshAccountsCoder;
    }
  | undefined;
let runtimeError: string | null = null;
let bootstrapPromise: Promise<StoredMarket> | undefined;

function stateDir(): string {
  return resolve(process.env.FORK_LAB_STATE_DIR ?? ".v2-fork-lab");
}

function statePath(): string {
  return resolve(process.env.FORK_LAB_STATE_PATH ?? `${stateDir()}/state.json`);
}

function ensureStateDir() {
  mkdirSync(stateDir(), { recursive: true, mode: 0o700 });
}

function readState(): ForkState {
  ensureStateDir();
  if (!existsSync(statePath())) return { markets: {} };
  return JSON.parse(readFileSync(statePath(), "utf8")) as ForkState;
}

function writeState(state: ForkState) {
  ensureStateDir();
  writeFileSync(statePath(), `${JSON.stringify(state, null, 2)}\n`, { mode: 0o600 });
}

function normalizePublicUrl(value: string): string {
  if (/^https?:\/\//i.test(value)) return value.replace(/\/$/, "");
  if (value.includes("localhost") || value.includes("127.0.0.1")) return `http://${value}`;
  return `https://${value}`;
}

function loadIdl(): anchor.Idl {
  const candidates = [
    process.env.OMNIPAIR_V2_IDL_PATH,
    "target/idl/omnipair_v2.json",
    "packages/program-interface/src/idl_v2.json",
  ].filter(Boolean) as string[];

  for (const candidate of candidates) {
    const path = resolve(candidate);
    if (existsSync(path)) {
      const idl = JSON.parse(readFileSync(path, "utf8"));
      return { ...idl, address: PROGRAM_ID.toBase58() } as anchor.Idl;
    }
  }

  throw new Error(
    `V2 IDL not found. Tried ${candidates.map((path) => resolve(path)).join(", ")}`
  );
}

function readKeypairFile(path: string): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
}

function parseKeypairSecret(value: string): Keypair {
  const trimmed = value.trim();
  const json = trimmed.startsWith("[")
    ? trimmed
    : Buffer.from(trimmed, "base64").toString("utf8");
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(json) as number[]));
}

function loadOrCreateKeypair(label: string): { keypair: Keypair; path: string; created: boolean } {
  ensureStateDir();
  const safeLabel = label.replace(/[^a-zA-Z0-9_-]/g, "-");
  const path = resolve(stateDir(), `${safeLabel}.json`);
  if (existsSync(path)) {
    return { keypair: readKeypairFile(path), path, created: false };
  }
  const keypair = Keypair.generate();
  writeFileSync(path, JSON.stringify(Array.from(keypair.secretKey)), { mode: 0o600 });
  return { keypair, path, created: true };
}

function loadPayer(): Keypair {
  const inline =
    process.env.FORK_LAB_PAYER_KEYPAIR_JSON ?? process.env.FORK_LAB_PAYER_KEYPAIR_BASE64;
  const materializedPath = resolve(
    process.env.FORK_LAB_MATERIALIZED_PAYER_PATH ?? `${stateDir()}/payer.json`
  );

  if (inline) {
    const payer = parseKeypairSecret(inline);
    mkdirSync(dirname(materializedPath), { recursive: true, mode: 0o700 });
    writeFileSync(materializedPath, JSON.stringify(Array.from(payer.secretKey)), { mode: 0o600 });
    return payer;
  }

  const keypairPath =
    process.env.FORK_LAB_PAYER_KEYPAIR ?? process.env.ANCHOR_WALLET ?? "deployer-keypair.json";
  const resolved = resolve(keypairPath);
  if (existsSync(resolved)) return readKeypairFile(resolved);

  return loadOrCreateKeypair("payer").keypair;
}

function initializeRuntime() {
  if (runtime) return runtime;

  try {
    const payer = loadPayer();
    const connection = new Connection(SURFPOOL_RPC_URL, "confirmed");
    const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
      skipPreflight: false,
    });
    const idl = loadIdl();
    const program = new anchor.Program({ ...idl, address: PROGRAM_ID.toBase58() } as any, provider);
    const accountCoder = new anchor.BorshAccountsCoder(idl);
    anchor.setProvider(provider);
    runtime = { payer, connection, provider, program, idl, accountCoder };
    runtimeError = null;
    return runtime;
  } catch (error) {
    runtimeError = error instanceof Error ? error.message : String(error);
    console.error("V2 fork API runtime initialization failed:", runtimeError);
    throw error;
  }
}

function seed(value: string): Buffer {
  return Buffer.from(value);
}

function pda(...seeds: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, PROGRAM_ID)[0];
}

function tokenMetadataPda(mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [seed("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    TOKEN_METADATA_PROGRAM_ID
  )[0];
}

function marketPda(baseMint: PublicKey, quoteMint: PublicKey, paramsHash: Buffer): PublicKey {
  return pda(seed("market_v2"), baseMint.toBuffer(), quoteMint.toBuffer(), paramsHash);
}

function deriveMarketAddresses(baseMint: PublicKey, quoteMint: PublicKey, paramsHash: Buffer) {
  const market = marketPda(baseMint, quoteMint, paramsHash);
  return {
    market,
    futarchyAuthority: pda(seed("futarchy_authority")),
    eventAuthority: pda(seed("__event_authority")),
    baseReserveVault: pda(seed("market_reserve"), market.toBuffer(), baseMint.toBuffer()),
    quoteReserveVault: pda(seed("market_reserve"), market.toBuffer(), quoteMint.toBuffer()),
    baseCollateralVault: pda(seed("market_collateral"), market.toBuffer(), baseMint.toBuffer()),
    quoteCollateralVault: pda(seed("market_collateral"), market.toBuffer(), quoteMint.toBuffer()),
    baseInsuranceVault: pda(seed("insurance"), market.toBuffer(), baseMint.toBuffer()),
    quoteInsuranceVault: pda(seed("insurance"), market.toBuffer(), quoteMint.toBuffer()),
    baseFeeVault: pda(seed("market_fee"), market.toBuffer(), baseMint.toBuffer()),
    quoteFeeVault: pda(seed("market_fee"), market.toBuffer(), quoteMint.toBuffer()),
    baseInterestVault: pda(seed("market_interest"), market.toBuffer(), baseMint.toBuffer()),
    quoteInterestVault: pda(seed("market_interest"), market.toBuffer(), quoteMint.toBuffer()),
  };
}

function deriveMarginPosition(market: PublicKey, owner: PublicKey): PublicKey {
  return pda(seed("margin"), market.toBuffer(), owner.toBuffer());
}

function deriveYieldAccount(
  market: PublicKey,
  owner: PublicKey,
  assetMint: PublicKey,
  tokenKind: YieldTokenKind
): PublicKey {
  return pda(
    seed("yield"),
    market.toBuffer(),
    owner.toBuffer(),
    assetMint.toBuffer(),
    Buffer.from([tokenKind === "ylp" ? 0 : 1])
  );
}

function deriveHlpYlpVault(
  market: PublicKey,
  targetHlpMint: PublicKey,
  ylpMint: PublicKey
): PublicKey {
  return pda(
    seed("hlp_ylp_vault"),
    market.toBuffer(),
    targetHlpMint.toBuffer(),
    ylpMint.toBuffer()
  );
}

function deriveProgramDataAddress(): PublicKey {
  return PublicKey.findProgramAddressSync([PROGRAM_ID.toBuffer()], BPF_LOADER_UPGRADEABLE_ID)[0];
}

function orderedMints(mintA: PublicKey, mintB: PublicKey): [PublicKey, PublicKey] {
  return Buffer.compare(mintA.toBuffer(), mintB.toBuffer()) < 0 ? [mintA, mintB] : [mintB, mintA];
}

function paramsHashForMarket(label: string, baseMint: PublicKey, quoteMint: PublicKey): Buffer {
  const override =
    process.env.OMNIPAIR_V2_FORK_PARAMS_HASH ?? process.env.OMNIPAIR_V2_MARKET_PARAMS_HASH;
  if (override) {
    const bytes = Buffer.from(override.replace(/^0x/, ""), "hex");
    if (bytes.length !== 32) throw new Error("OMNIPAIR_V2_FORK_PARAMS_HASH must be 32 bytes");
    return bytes;
  }
  return createHash("sha256")
    .update(`omnipair-v2-mainnet-fork:${label}:${baseMint.toBase58()}:${quoteMint.toBase58()}`)
    .digest();
}

function toBN(value: bigint | number | string): BN {
  return new BN(value.toString());
}

function toBigInt(value: BN | bigint | number | string | null | undefined): bigint {
  if (value == null) return 0n;
  if (typeof value === "bigint") return value;
  if (typeof value === "number") return BigInt(value);
  if (typeof value === "string") return BigInt(value);
  return BigInt(value.toString());
}

function stringValue(value: unknown): string {
  if (value == null) return "0";
  if (typeof value === "bigint") return value.toString();
  if (typeof value === "number") return String(value);
  if (typeof value === "string") return value;
  if (value instanceof PublicKey) return value.toBase58();
  if (value instanceof BN) return value.toString();
  if (typeof value === "object" && value.constructor?.name === "BN") {
    return (value as { toString(): string }).toString();
  }
  return String(value);
}

function field<T = unknown>(obj: any, camel: string, snake?: string): T {
  if (!obj) return undefined as T;
  if (obj[camel] !== undefined) return obj[camel] as T;
  if (snake && obj[snake] !== undefined) return obj[snake] as T;
  return undefined as T;
}

function parseUnits(value: string | number | bigint | undefined, decimals: number): bigint {
  if (typeof value === "bigint") return value;
  const raw = String(value ?? "0").trim();
  if (!/^\d+(\.\d+)?$/.test(raw)) throw new Error(`Invalid decimal amount: ${raw}`);
  const [whole, fraction = ""] = raw.split(".");
  const normalizedFraction = fraction.padEnd(decimals, "0").slice(0, decimals);
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(normalizedFraction || "0");
}

async function rpcRequest(method: string, params: unknown[]) {
  const response = await fetch(SURFPOOL_RPC_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  const payload = (await response.json()) as { result?: unknown; error?: unknown };
  if (payload.error) throw new Error(`${method} failed: ${JSON.stringify(payload.error)}`);
  return payload.result;
}

async function setLamports(pubkey: PublicKey, sol: number) {
  const { connection } = initializeRuntime();
  try {
    const signature = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(signature, "confirmed");
  } catch {
    await rpcRequest("surfnet_setAccount", [
      pubkey.toBase58(),
      {
        lamports: sol * LAMPORTS_PER_SOL,
        owner: SystemProgram.programId.toBase58(),
      },
    ]);
  }
}

async function setTokenBalance(
  owner: PublicKey,
  mint: PublicKey,
  amount: bigint,
  tokenProgram: PublicKey
) {
  if (amount > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`surfnet_setTokenAccount amount is above JSON safe integer range: ${amount}`);
  }
  await rpcRequest("surfnet_setTokenAccount", [
    owner.toBase58(),
    mint.toBase58(),
    {
      amount: Number(amount),
      state: "initialized",
    },
    tokenProgram.toBase58(),
  ]);
}

async function setRawAccount(params: {
  pubkey: PublicKey;
  owner: PublicKey;
  lamports: number;
  data: Buffer;
}) {
  const account = {
    lamports: params.lamports,
    owner: params.owner.toBase58(),
    executable: false,
    data: params.data.toString("hex"),
  };
  await rpcRequest("surfnet_setAccount", [params.pubkey.toBase58(), account]);
}

async function tokenProgramForMint(mint: PublicKey): Promise<PublicKey> {
  const { connection } = initializeRuntime();
  const account = await connection.getAccountInfo(mint, "confirmed");
  if (!account) throw new Error(`Mint account not found in fork: ${mint.toBase58()}`);
  return account.owner.equals(TOKEN_2022_PROGRAM_ID) ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
}

async function mintDecimals(mint: PublicKey, tokenProgram?: PublicKey): Promise<number> {
  const { connection } = initializeRuntime();
  const programId = tokenProgram ?? (await tokenProgramForMint(mint));
  return (await getMint(connection, mint, "confirmed", programId)).decimals;
}

async function tokenAccountAmount(tokenAccount: PublicKey, tokenProgram: PublicKey): Promise<bigint> {
  const { connection } = initializeRuntime();
  try {
    return (await getAccount(connection, tokenAccount, "confirmed", tokenProgram)).amount;
  } catch {
    return 0n;
  }
}

async function ataInstructionIfMissing(params: {
  payer: PublicKey;
  owner: PublicKey;
  mint: PublicKey;
  tokenProgram: PublicKey;
  allowOwnerOffCurve?: boolean;
}): Promise<{ address: PublicKey; instruction?: TransactionInstruction }> {
  const { connection } = initializeRuntime();
  const address = getAssociatedTokenAddressSync(
    params.mint,
    params.owner,
    params.allowOwnerOffCurve ?? false,
    params.tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const existing = await connection.getAccountInfo(address, "confirmed");
  if (existing) return { address };
  return {
    address,
    instruction: createAssociatedTokenAccountInstruction(
      params.payer,
      address,
      params.owner,
      params.mint,
      params.tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    ),
  };
}

async function createAtaIfMissing(params: {
  payer: Keypair;
  owner: PublicKey;
  mint: PublicKey;
  tokenProgram: PublicKey;
  allowOwnerOffCurve?: boolean;
}): Promise<PublicKey> {
  const { provider } = initializeRuntime();
  const ata = await ataInstructionIfMissing({
    payer: params.payer.publicKey,
    owner: params.owner,
    mint: params.mint,
    tokenProgram: params.tokenProgram,
    allowOwnerOffCurve: params.allowOwnerOffCurve,
  });
  if (ata.instruction) {
    await provider.sendAndConfirm(new Transaction().add(ata.instruction), [params.payer]);
  }
  return ata.address;
}

function defaultMarketConfig() {
  return {
    swapFeeBps: Number(process.env.OMNIPAIR_V2_SWAP_FEE_BPS ?? "30"),
    operatorFeeBps: Number(process.env.OMNIPAIR_V2_OPERATOR_FEE_BPS ?? "0"),
    protocolFeeBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_FEE_BPS ?? "0"),
    targetHlpLeverageBps: Number(process.env.OMNIPAIR_V2_TARGET_HLP_LEVERAGE_BPS ?? "20000"),
    settlementDivergenceBps: Number(process.env.OMNIPAIR_V2_SETTLEMENT_DIVERGENCE_BPS ?? "500"),
    emergencyExitHaircutBps: Number(process.env.OMNIPAIR_V2_EMERGENCY_EXIT_HAIRCUT_BPS ?? "250"),
    emaHalfLifeMs: toBN(process.env.OMNIPAIR_V2_EMA_HALF_LIFE_MS ?? "60000"),
    directionalEmaHalfLifeMs: toBN(
      process.env.OMNIPAIR_V2_DIRECTIONAL_EMA_HALF_LIFE_MS ?? "60000"
    ),
    kEmaHalfLifeMs: toBN(process.env.OMNIPAIR_V2_K_EMA_HALF_LIFE_MS ?? "60000"),
    maxDailyBorrowBps: Number(process.env.OMNIPAIR_V2_MAX_DAILY_BORROW_BPS ?? "2000"),
    maxDailyWithdrawBps: Number(process.env.OMNIPAIR_V2_MAX_DAILY_WITHDRAW_BPS ?? "2000"),
    spotEmaDivergenceBps: Number(process.env.OMNIPAIR_V2_SPOT_EMA_DIVERGENCE_BPS ?? "1000"),
    kEmaDrawdownBps: Number(process.env.OMNIPAIR_V2_K_EMA_DRAWDOWN_BPS ?? "1000"),
    recognizedCollateralCapBps: Number(
      process.env.OMNIPAIR_V2_RECOGNIZED_COLLATERAL_CAP_BPS ?? "15000"
    ),
    marketHealthMinBps: Number(process.env.OMNIPAIR_V2_MARKET_HEALTH_MIN_BPS ?? "11000"),
    liquidationAuctionDurationSlots: toBN(
      process.env.OMNIPAIR_V2_LIQUIDATION_AUCTION_DURATION_SLOTS ?? "1200"
    ),
    liquidationAuctionStartIncentiveBps: Number(
      process.env.OMNIPAIR_V2_LIQUIDATION_AUCTION_START_INCENTIVE_BPS ?? "0"
    ),
    hedgedLpEnabled: process.env.OMNIPAIR_V2_HEDGED_LP_ENABLED !== "0",
    startTime: toBN(process.env.OMNIPAIR_V2_MARKET_START_TIME ?? "0"),
  };
}

function defaultLpMetadata(kind: "ylp" | "baseHlp" | "quoteHlp") {
  const prefix =
    kind === "ylp"
      ? "OMNIPAIR_V2_YLP"
      : kind === "baseHlp"
        ? "OMNIPAIR_V2_BASE_HLP"
        : "OMNIPAIR_V2_QUOTE_HLP";
  const defaults = {
    ylp: {
      name: "Omnipair Dusk yLP",
      symbol: "yLP",
      uri: "https://omnipair.fi/metadata/dusk/ylp.json",
    },
    baseHlp: {
      name: "Omnipair Dusk Base hLP",
      symbol: "hLP",
      uri: "https://omnipair.fi/metadata/dusk/base-hlp.json",
    },
    quoteHlp: {
      name: "Omnipair Dusk Quote hLP",
      symbol: "hLP",
      uri: "https://omnipair.fi/metadata/dusk/quote-hlp.json",
    },
  }[kind];
  return {
    name: process.env[`${prefix}_NAME`] ?? defaults.name,
    symbol: process.env[`${prefix}_SYMBOL`] ?? defaults.symbol,
    uri: process.env[`${prefix}_URI`] ?? defaults.uri,
  };
}

async function ensureFutarchyAuthority(futarchyAuthority: PublicKey) {
  const { program, payer, accountCoder, connection } = initializeRuntime();
  const existing = await program.account.futarchyAuthority.fetchNullable(futarchyAuthority);
  if (existing) return existing;

  await setLamports(payer.publicKey, DEFAULT_SOL_FUNDING);

  try {
    const signature = await program.methods
      .initFutarchyAuthority({
        authority: payer.publicKey,
        swapBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_SWAP_BPS ?? "0"),
        interestBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_INTEREST_BPS ?? "0"),
        futarchyTreasury: payer.publicKey,
        futarchyTreasuryBps: 0,
        buybacksVault: payer.publicKey,
        buybacksVaultBps: 0,
        teamTreasury: payer.publicKey,
        teamTreasuryBps: 10_000,
      })
      .accounts({
        deployer: payer.publicKey,
        futarchyAuthority,
        programData: deriveProgramDataAddress(),
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log(`V2 futarchy authority initialized: ${signature}`);
    return await program.account.futarchyAuthority.fetch(futarchyAuthority);
  } catch (error) {
    console.warn(
      `initFutarchyAuthority failed; seeding authority account through Surfpool: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }

  const [, bump] = PublicKey.findProgramAddressSync([seed("futarchy_authority")], PROGRAM_ID);
  const data = await accountCoder.encode("FutarchyAuthority", {
    version: 1,
    authority: payer.publicKey,
    recipients: {
      futarchy_treasury: payer.publicKey,
      buybacks_vault: payer.publicKey,
      team_treasury: payer.publicKey,
    },
    revenue_share: {
      swap_bps: Number(process.env.OMNIPAIR_V2_PROTOCOL_SWAP_BPS ?? "0"),
      interest_bps: Number(process.env.OMNIPAIR_V2_PROTOCOL_INTEREST_BPS ?? "0"),
    },
    revenue_distribution: {
      futarchy_treasury_bps: 0,
      buybacks_vault_bps: 0,
      team_treasury_bps: 10_000,
    },
    global_reduce_only: false,
    bump,
  });
  await setRawAccount({
    pubkey: futarchyAuthority,
    owner: PROGRAM_ID,
    lamports: await connection.getMinimumBalanceForRentExemption(data.length),
    data,
  });
  return await program.account.futarchyAuthority.fetch(futarchyAuthority);
}

async function createHookedLpMintIfMissing(params: {
  label: string;
  decimals: number;
  mintAuthority: PublicKey;
}) {
  const { connection, payer } = initializeRuntime();
  const { keypair, path } = loadOrCreateKeypair(`mint-${params.label}`);
  const existing = await connection.getAccountInfo(keypair.publicKey, "confirmed");
  if (!existing) {
    await setLamports(payer.publicKey, DEFAULT_SOL_FUNDING);
    const mintLen = getMintLen([ExtensionType.TransferHook]);
    const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);
    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: keypair.publicKey,
        lamports,
        space: mintLen,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        keypair.publicKey,
        payer.publicKey,
        PROGRAM_ID,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        keypair.publicKey,
        params.decimals,
        params.mintAuthority,
        null,
        TOKEN_2022_PROGRAM_ID
      )
    );
    transaction.feePayer = payer.publicKey;
    await anchor.web3.sendAndConfirmTransaction(connection, transaction, [payer, keypair], {
      commitment: "confirmed",
    });
  }
  return {
    mint: keypair.publicKey,
    keypairPath: path,
  };
}

type TransferHookSeed =
  | { kind: "literal"; bytes: Uint8Array | Buffer | number[] }
  | { kind: "accountKey"; index: number }
  | { kind: "accountData"; accountIndex: number; dataIndex: number; length: number };

function packTransferHookSeedConfig(seeds: TransferHookSeed[]): Buffer {
  const config = Buffer.alloc(32);
  let offset = 0;
  const write = (value: number) => {
    if (value < 0 || value > 255 || !Number.isInteger(value)) {
      throw new Error(`transfer-hook seed byte out of range: ${value}`);
    }
    if (offset >= config.length) throw new Error("transfer-hook seed config exceeds 32 bytes");
    config[offset] = value;
    offset += 1;
  };

  for (const transferSeed of seeds) {
    if (transferSeed.kind === "literal") {
      const bytes = Buffer.from(transferSeed.bytes);
      write(1);
      write(bytes.length);
      if (offset + bytes.length > config.length) {
        throw new Error("transfer-hook seed config exceeds 32 bytes");
      }
      bytes.copy(config, offset);
      offset += bytes.length;
    } else if (transferSeed.kind === "accountKey") {
      write(3);
      write(transferSeed.index);
    } else {
      write(4);
      write(transferSeed.accountIndex);
      write(transferSeed.dataIndex);
      write(transferSeed.length);
    }
  }

  return config;
}

function encodeTransferHookValidationAccountData(params: {
  market: PublicKey;
  assetMint: PublicKey;
  tokenKind: YieldTokenKind;
}): Buffer {
  const executeDiscriminator = Buffer.from([105, 37, 101, 197, 75, 251, 102, 26]);
  const metas = [
    { discriminator: 0, addressConfig: params.market.toBuffer(), isSigner: false, isWritable: false },
    {
      discriminator: 0,
      addressConfig: params.assetMint.toBuffer(),
      isSigner: false,
      isWritable: false,
    },
    {
      discriminator: 1,
      addressConfig: packTransferHookSeedConfig([
        { kind: "literal", bytes: seed("yield") },
        { kind: "accountKey", index: 5 },
        { kind: "accountData", accountIndex: 0, dataIndex: 32, length: 32 },
        { kind: "accountKey", index: 6 },
        { kind: "literal", bytes: [params.tokenKind === "ylp" ? 0 : 1] },
      ]),
      isSigner: false,
      isWritable: true,
    },
    {
      discriminator: 1,
      addressConfig: packTransferHookSeedConfig([
        { kind: "literal", bytes: seed("yield") },
        { kind: "accountKey", index: 5 },
        { kind: "accountData", accountIndex: 2, dataIndex: 32, length: 32 },
        { kind: "accountKey", index: 6 },
        { kind: "literal", bytes: [params.tokenKind === "ylp" ? 0 : 1] },
      ]),
      isSigner: false,
      isWritable: true,
    },
  ];

  const podSliceLength = 4 + metas.length * 35;
  const data = Buffer.alloc(8 + 4 + podSliceLength);
  executeDiscriminator.copy(data, 0);
  data.writeUInt32LE(podSliceLength, 8);
  data.writeUInt32LE(metas.length, 12);
  let offset = 16;
  for (const meta of metas) {
    data[offset] = meta.discriminator;
    offset += 1;
    meta.addressConfig.copy(data, offset);
    offset += 32;
    data[offset] = meta.isSigner ? 1 : 0;
    offset += 1;
    data[offset] = meta.isWritable ? 1 : 0;
    offset += 1;
  }
  return data;
}

function encodeYlpTransferHookValidationAccountData(params: {
  market: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
}): Buffer {
  const executeDiscriminator = Buffer.from([105, 37, 101, 197, 75, 251, 102, 26]);
  const ylpSeed = (ownerAccountIndex: number, assetMintAccountIndex: number) =>
    packTransferHookSeedConfig([
      { kind: "literal", bytes: seed("yield") },
      { kind: "accountKey", index: 5 },
      { kind: "accountData", accountIndex: ownerAccountIndex, dataIndex: 32, length: 32 },
      { kind: "accountKey", index: assetMintAccountIndex },
      { kind: "literal", bytes: [0] },
    ]);
  const metas = [
    { discriminator: 0, addressConfig: params.market.toBuffer(), isSigner: false, isWritable: false },
    { discriminator: 0, addressConfig: params.baseMint.toBuffer(), isSigner: false, isWritable: false },
    { discriminator: 0, addressConfig: params.quoteMint.toBuffer(), isSigner: false, isWritable: false },
    { discriminator: 1, addressConfig: ylpSeed(0, 6), isSigner: false, isWritable: true },
    { discriminator: 1, addressConfig: ylpSeed(2, 6), isSigner: false, isWritable: true },
    { discriminator: 1, addressConfig: ylpSeed(0, 7), isSigner: false, isWritable: true },
    { discriminator: 1, addressConfig: ylpSeed(2, 7), isSigner: false, isWritable: true },
  ];

  const podSliceLength = 4 + metas.length * 35;
  const data = Buffer.alloc(8 + 4 + podSliceLength);
  executeDiscriminator.copy(data, 0);
  data.writeUInt32LE(podSliceLength, 8);
  data.writeUInt32LE(metas.length, 12);
  let offset = 16;
  for (const meta of metas) {
    data[offset] = meta.discriminator;
    offset += 1;
    meta.addressConfig.copy(data, offset);
    offset += 32;
    data[offset] = meta.isSigner ? 1 : 0;
    offset += 1;
    data[offset] = meta.isWritable ? 1 : 0;
    offset += 1;
  }
  return data;
}

function deriveTransferHookValidationAddress(lpMint: PublicKey): PublicKey {
  return pda(seed("extra-account-metas"), lpMint.toBuffer());
}

async function seedTransferHookValidationAccount(params: {
  lpMint: PublicKey;
  market: PublicKey;
  assetMint: PublicKey;
  tokenKind: YieldTokenKind;
}) {
  const { connection } = initializeRuntime();
  const validationAccount = deriveTransferHookValidationAddress(params.lpMint);
  if (await connection.getAccountInfo(validationAccount, "confirmed")) return validationAccount;
  const data = encodeTransferHookValidationAccountData(params);
  try {
    await setRawAccount({
      pubkey: validationAccount,
      owner: PROGRAM_ID,
      lamports: await connection.getMinimumBalanceForRentExemption(data.length),
      data,
    });
  } catch (error) {
    console.warn(
      `Unable to seed transfer-hook validation account ${validationAccount.toBase58()}: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }
  return validationAccount;
}

async function seedYlpTransferHookValidationAccount(params: {
  lpMint: PublicKey;
  market: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
}) {
  const { connection } = initializeRuntime();
  const validationAccount = deriveTransferHookValidationAddress(params.lpMint);
  if (await connection.getAccountInfo(validationAccount, "confirmed")) return validationAccount;
  const data = encodeYlpTransferHookValidationAccountData(params);
  try {
    await setRawAccount({
      pubkey: validationAccount,
      owner: PROGRAM_ID,
      lamports: await connection.getMinimumBalanceForRentExemption(data.length),
      data,
    });
  } catch (error) {
    console.warn(
      `Unable to seed transfer-hook validation account ${validationAccount.toBase58()}: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }
  return validationAccount;
}

async function ensureLpMetadata(params: {
  market: PublicKey;
  lpMint: PublicKey;
  lpTokenMetadata: PublicKey;
  metadata: { name: string; symbol: string; uri: string };
}) {
  const { connection, program, payer } = initializeRuntime();
  if (await connection.getAccountInfo(params.lpTokenMetadata, "confirmed")) return;

  const signature = await program.methods
    .initializeLpMetadata(params.metadata)
    .accounts({
      payer: payer.publicKey,
      market: params.market,
      lpMint: params.lpMint,
      lpTokenMetadata: params.lpTokenMetadata,
      systemProgram: SystemProgram.programId,
      tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    })
    .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 250_000 })])
    .rpc();
  console.log(`V2 fork LP metadata initialized: ${signature}`);
}

async function bootstrap(): Promise<StoredMarket> {
  initializeRuntime();
  bootstrapPromise ??= bootstrapUncached();
  return bootstrapPromise;
}

async function bootstrapUncached(): Promise<StoredMarket> {
  const { connection, payer, program } = initializeRuntime();
  const state = readState();
  const marketLabel = process.env.OMNIPAIR_V2_MARKET_LABEL ?? "meta-usdc-mainnet-fork";
  const defaultBase = new PublicKey(process.env.OMNIPAIR_V2_BASE_MINT ?? DEFAULT_META_MINT);
  const defaultQuote = new PublicKey(process.env.OMNIPAIR_V2_QUOTE_MINT ?? DEFAULT_USDC_MINT);
  const [baseMint, quoteMint] = orderedMints(defaultBase, defaultQuote);
  const paramsHash = paramsHashForMarket(marketLabel, baseMint, quoteMint);
  const addresses = deriveMarketAddresses(baseMint, quoteMint, paramsHash);

  await setLamports(payer.publicKey, DEFAULT_SOL_FUNDING);

  const [baseTokenProgram, quoteTokenProgram] = await Promise.all([
    tokenProgramForMint(baseMint),
    tokenProgramForMint(quoteMint),
  ]);
  const [baseDecimals, quoteDecimals] = await Promise.all([
    mintDecimals(baseMint, baseTokenProgram),
    mintDecimals(quoteMint, quoteTokenProgram),
  ]);

  const futarchy = await ensureFutarchyAuthority(addresses.futarchyAuthority);
  const teamTreasury =
    field<PublicKey>(field(futarchy, "recipients"), "teamTreasury", "team_treasury") ??
    payer.publicKey;
  const teamTreasuryWsolAccount = await createAtaIfMissing({
    payer,
    owner: teamTreasury,
    mint: NATIVE_MINT,
    tokenProgram: TOKEN_PROGRAM_ID,
    allowOwnerOffCurve: true,
  });

  const lpLabels = {
    ylp: `${marketLabel}-ylp`,
    baseHlp: `${marketLabel}-base-hlp`,
    quoteHlp: `${marketLabel}-quote-hlp`,
  };

  const existingMarketAccount = await program.account.market.fetchNullable(addresses.market);
  let ylpMint = field<PublicKey>(existingMarketAccount, "ylpMint", "ylp_mint");
  const existingBaseSide = field<any>(existingMarketAccount, "baseSide", "base_side");
  const existingQuoteSide = field<any>(existingMarketAccount, "quoteSide", "quote_side");
  let baseHlpMint = field<PublicKey>(existingBaseSide, "hlpMint", "hlp_mint");
  let quoteHlpMint = field<PublicKey>(existingQuoteSide, "hlpMint", "hlp_mint");

  if (!existingMarketAccount) {
    const [ylp, baseHlp, quoteHlp] = await Promise.all([
      createHookedLpMintIfMissing({
        label: lpLabels.ylp,
        decimals: baseDecimals,
        mintAuthority: addresses.market,
      }),
      createHookedLpMintIfMissing({
        label: lpLabels.baseHlp,
        decimals: baseDecimals,
        mintAuthority: addresses.market,
      }),
      createHookedLpMintIfMissing({
        label: lpLabels.quoteHlp,
        decimals: quoteDecimals,
        mintAuthority: addresses.market,
      }),
    ]);
    ylpMint = ylp.mint;
    baseHlpMint = baseHlp.mint;
    quoteHlpMint = quoteHlp.mint;
  }

  if (!ylpMint || !baseHlpMint || !quoteHlpMint) {
    throw new Error(`Unable to resolve V2 LP mints for market ${addresses.market.toBase58()}`);
  }

  const ylpTokenMetadata = tokenMetadataPda(ylpMint);
  const baseHlpTokenMetadata = tokenMetadataPda(baseHlpMint);
  const quoteHlpTokenMetadata = tokenMetadataPda(quoteHlpMint);
  const baseHlpYlpVault = deriveHlpYlpVault(addresses.market, baseHlpMint, ylpMint);
  const quoteHlpYlpVault = deriveHlpYlpVault(addresses.market, quoteHlpMint, ylpMint);

  const transferHookValidationAccounts = {
    ylp: (
      await seedYlpTransferHookValidationAccount({
        lpMint: ylpMint,
        market: addresses.market,
        baseMint,
        quoteMint,
      })
    ).toBase58(),
    baseHlp: (
      await seedTransferHookValidationAccount({
        lpMint: baseHlpMint,
        market: addresses.market,
        assetMint: baseMint,
        tokenKind: "hlp",
      })
    ).toBase58(),
    quoteHlp: (
      await seedTransferHookValidationAccount({
        lpMint: quoteHlpMint,
        market: addresses.market,
        assetMint: quoteMint,
        tokenKind: "hlp",
      })
    ).toBase58(),
  };

  if (!existingMarketAccount) {
    const signature = await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: field<PublicKey>(futarchy, "authority") ?? payer.publicKey,
        config: defaultMarketConfig(),
        paramsHash: Array.from(paramsHash),
      })
      .accounts({
        payer: payer.publicKey,
        baseMint,
        quoteMint,
        market: addresses.market,
        futarchyAuthority: addresses.futarchyAuthority,
        ylpMint,
        baseHlpMint,
        quoteHlpMint,
        baseReserveVault: addresses.baseReserveVault,
        quoteReserveVault: addresses.quoteReserveVault,
        baseCollateralVault: addresses.baseCollateralVault,
        quoteCollateralVault: addresses.quoteCollateralVault,
        baseInsuranceVault: addresses.baseInsuranceVault,
        quoteInsuranceVault: addresses.quoteInsuranceVault,
        baseFeeVault: addresses.baseFeeVault,
        quoteFeeVault: addresses.quoteFeeVault,
        baseInterestVault: addresses.baseInterestVault,
        quoteInterestVault: addresses.quoteInterestVault,
        teamTreasury,
        teamTreasuryWsolAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: addresses.eventAuthority,
        program: PROGRAM_ID,
      })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();
    console.log(`V2 fork market initialized: ${signature}`);
  }

  await ensureLpMetadata({
    market: addresses.market,
    lpMint: ylpMint,
    lpTokenMetadata: ylpTokenMetadata,
    metadata: defaultLpMetadata("ylp"),
  });
  await ensureLpMetadata({
    market: addresses.market,
    lpMint: baseHlpMint,
    lpTokenMetadata: baseHlpTokenMetadata,
    metadata: defaultLpMetadata("baseHlp"),
  });
  await ensureLpMetadata({
    market: addresses.market,
    lpMint: quoteHlpMint,
    lpTokenMetadata: quoteHlpTokenMetadata,
    metadata: defaultLpMetadata("quoteHlp"),
  });

  const previous = state.markets[marketLabel];
  const stored: StoredMarket = {
    label: marketLabel,
    programId: PROGRAM_ID.toBase58(),
    market: addresses.market.toBase58(),
    paramsHash: paramsHash.toString("hex"),
    baseMint: baseMint.toBase58(),
    quoteMint: quoteMint.toBase58(),
    baseDecimals,
    quoteDecimals,
    baseTokenProgram: baseTokenProgram.toBase58(),
    quoteTokenProgram: quoteTokenProgram.toBase58(),
    ylpMint: ylpMint.toBase58(),
    baseHlpMint: baseHlpMint.toBase58(),
    quoteHlpMint: quoteHlpMint.toBase58(),
    ylpTokenMetadata: ylpTokenMetadata.toBase58(),
    baseHlpTokenMetadata: baseHlpTokenMetadata.toBase58(),
    quoteHlpTokenMetadata: quoteHlpTokenMetadata.toBase58(),
    baseReserveVault: addresses.baseReserveVault.toBase58(),
    quoteReserveVault: addresses.quoteReserveVault.toBase58(),
    baseCollateralVault: addresses.baseCollateralVault.toBase58(),
    quoteCollateralVault: addresses.quoteCollateralVault.toBase58(),
    baseInsuranceVault: addresses.baseInsuranceVault.toBase58(),
    quoteInsuranceVault: addresses.quoteInsuranceVault.toBase58(),
    baseFeeVault: addresses.baseFeeVault.toBase58(),
    quoteFeeVault: addresses.quoteFeeVault.toBase58(),
    baseInterestVault: addresses.baseInterestVault.toBase58(),
    quoteInterestVault: addresses.quoteInterestVault.toBase58(),
    baseHlpYlpVault: baseHlpYlpVault.toBase58(),
    quoteHlpYlpVault: quoteHlpYlpVault.toBase58(),
    eventAuthority: addresses.eventAuthority.toBase58(),
    seededLiquidity: previous?.market === addresses.market.toBase58() && previous.seededLiquidity,
    transferHookValidationAccounts,
  };

  state.markets[marketLabel] = stored;
  writeState(state);

  if (process.env.OMNIPAIR_V2_SEED_LIQUIDITY !== "0" && !stored.seededLiquidity) {
    await seedInitialLiquidity(stored);
    stored.seededLiquidity = true;
    state.markets[marketLabel] = stored;
    writeState(state);
  }

  return stored;
}

async function seedInitialLiquidity(market: StoredMarket) {
  const { provider, payer } = initializeRuntime();
  const baseMint = new PublicKey(market.baseMint);
  const quoteMint = new PublicKey(market.quoteMint);
  const baseAmount = parseUnits(DEFAULT_SEED_BASE_UI, market.baseDecimals);
  const quoteAmount = parseUnits(DEFAULT_SEED_QUOTE_UI, market.quoteDecimals);
  const baseProgram = new PublicKey(market.baseTokenProgram);
  const quoteProgram = new PublicKey(market.quoteTokenProgram);

  await setTokenBalance(payer.publicKey, baseMint, baseAmount, baseProgram);
  await setTokenBalance(payer.publicKey, quoteMint, quoteAmount, quoteProgram);
  const tx = await buildAddLiquidityTx({
    owner: payer.publicKey,
    market,
    baseDepositAmount: baseAmount,
    quoteDepositAmount: quoteAmount,
    minYlpAmount: 0n,
    payerCanSign: true,
  });
  tx.sign(payer);
  const signature = await provider.connection.sendRawTransaction(tx.serialize());
  await provider.connection.confirmTransaction(signature, "confirmed");
  console.log(`V2 fork market seeded with initial liquidity: ${signature}`);
}

function marketConfigPayload(marketAccount: any) {
  const config = field<any>(marketAccount, "config");
  return {
    targetHlpLeverageBps: Number(field(config, "targetHlpLeverageBps", "target_hlp_leverage_bps") ?? 0),
    swapFeeBps: Number(field(config, "swapFeeBps", "swap_fee_bps") ?? 0),
    operatorFeeBps: Number(field(config, "operatorFeeBps", "operator_fee_bps") ?? 0),
    protocolFeeBps: Number(field(config, "protocolFeeBps", "protocol_fee_bps") ?? 0),
  };
}

async function marketPayload(stored: StoredMarket) {
  const { program } = initializeRuntime();
  const marketAccount = await program.account.market.fetch(new PublicKey(stored.market));
  const config = marketConfigPayload(marketAccount);
  const baseSide = field<any>(marketAccount, "baseSide", "base_side");
  const quoteSide = field<any>(marketAccount, "quoteSide", "quote_side");
  const debt = field<any>(marketAccount, "debt");
  const health = field<any>(marketAccount, "health");
  const now = new Date().toISOString();

  return {
    marketAddress: stored.market,
    baseMint: stored.baseMint,
    quoteMint: stored.quoteMint,
    baseDecimals: stored.baseDecimals,
    quoteDecimals: stored.quoteDecimals,
    ylpMint: stored.ylpMint,
    baseHlpMint: stored.baseHlpMint,
    quoteHlpMint: stored.quoteHlpMint,
    baseReserveVault: stored.baseReserveVault,
    quoteReserveVault: stored.quoteReserveVault,
    baseCollateralVault: stored.baseCollateralVault,
    quoteCollateralVault: stored.quoteCollateralVault,
    baseInsuranceVault: stored.baseInsuranceVault,
    quoteInsuranceVault: stored.quoteInsuranceVault,
    operator: stringValue(field(marketAccount, "operator")),
    manager: stringValue(field(marketAccount, "manager")),
    targetHlpLeverageBps: config.targetHlpLeverageBps,
    swapFeeBps: config.swapFeeBps,
    operatorFeeBps: config.operatorFeeBps,
    protocolFeeBps: config.protocolFeeBps,
    paramsHash: stored.paramsHash,
    version: Number(field(marketAccount, "version") ?? 1),
    reduceOnly: Boolean(field(marketAccount, "reduceOnly", "reduce_only") ?? false),
    createdTxSig: null,
    createdSlot: null,
    createdAt: now,
    updatedAt: now,
    swapCount: 0,
    lastSwapAt: null,
    state: {
      baseReserve: stringValue(field(field(baseSide, "reserves"), "liveReserve", "live_reserve")),
      quoteReserve: stringValue(field(field(quoteSide, "reserves"), "liveReserve", "live_reserve")),
      baseReserveYlpSupply: stringValue(field(field(baseSide, "shares"), "ylpSupply", "ylp_supply")),
      quoteReserveYlpSupply: stringValue(field(field(quoteSide, "shares"), "ylpSupply", "ylp_supply")),
      fixedBaseDebt: stringValue(field(debt, "fixedBaseShares", "fixed_base_shares")),
      fixedQuoteDebt: stringValue(field(debt, "fixedQuoteShares", "fixed_quote_shares")),
      recognizedBaseCollateralForQuoteDebt: stringValue(
        field(
          health,
          "recognizedBaseCollateralForQuoteDebt",
          "recognized_base_collateral_for_quote_debt"
        )
      ),
      recognizedQuoteCollateralForBaseDebt: stringValue(
        field(
          health,
          "recognizedQuoteCollateralForBaseDebt",
          "recognized_quote_collateral_for_base_debt"
        )
      ),
      effectiveBaseDebtNad: stringValue(field(health, "effectiveBaseDebtNad", "effective_base_debt_nad")),
      effectiveQuoteDebtNad: stringValue(
        field(health, "effectiveQuoteDebtNad", "effective_quote_debt_nad")
      ),
      baseDebtHealthBps: stringValue(field(health, "baseDebtHealthBps", "base_debt_health_bps")),
      quoteDebtHealthBps: stringValue(field(health, "quoteDebtHealthBps", "quote_debt_health_bps")),
      sourceTxSig: null,
      sourceSlot: null,
      updatedAt: now,
    },
  };
}

function forkConfigPayload(stored: StoredMarket) {
  return {
    rpcUrl: PUBLIC_RPC_URL,
    privateRpcUrl: SURFPOOL_RPC_URL,
    programId: PROGRAM_ID.toBase58(),
    payer: initializeRuntime().payer.publicKey.toBase58(),
    market: stored.market,
    label: stored.label,
    baseMint: stored.baseMint,
    quoteMint: stored.quoteMint,
    baseDecimals: stored.baseDecimals,
    quoteDecimals: stored.quoteDecimals,
    baseTokenProgram: stored.baseTokenProgram,
    quoteTokenProgram: stored.quoteTokenProgram,
    ylpMint: stored.ylpMint,
    baseHlpMint: stored.baseHlpMint,
    quoteHlpMint: stored.quoteHlpMint,
    seededLiquidity: stored.seededLiquidity,
    transferHookValidationAccounts: stored.transferHookValidationAccounts,
  };
}

function marketFromStored(stored: StoredMarket) {
  return {
    market: new PublicKey(stored.market),
    futarchyAuthority: pda(seed("futarchy_authority")),
    eventAuthority: new PublicKey(stored.eventAuthority),
    baseMint: new PublicKey(stored.baseMint),
    quoteMint: new PublicKey(stored.quoteMint),
    baseTokenProgram: new PublicKey(stored.baseTokenProgram),
    quoteTokenProgram: new PublicKey(stored.quoteTokenProgram),
    ylpMint: new PublicKey(stored.ylpMint),
    baseHlpMint: new PublicKey(stored.baseHlpMint),
    quoteHlpMint: new PublicKey(stored.quoteHlpMint),
    baseReserveVault: new PublicKey(stored.baseReserveVault),
    quoteReserveVault: new PublicKey(stored.quoteReserveVault),
    baseCollateralVault: new PublicKey(stored.baseCollateralVault),
    quoteCollateralVault: new PublicKey(stored.quoteCollateralVault),
    baseFeeVault: new PublicKey(stored.baseFeeVault),
    quoteFeeVault: new PublicKey(stored.quoteFeeVault),
    baseInterestVault: new PublicKey(stored.baseInterestVault),
    quoteInterestVault: new PublicKey(stored.quoteInterestVault),
    baseHlpYlpVault: new PublicKey(stored.baseHlpYlpVault),
    quoteHlpYlpVault: new PublicKey(stored.quoteHlpYlpVault),
  };
}

async function ownerTransaction(
  owner: PublicKey,
  instructions: TransactionInstruction[],
  payerCanSign = false
): Promise<Transaction> {
  const { connection, payer } = initializeRuntime();
  const tx = new Transaction();
  tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }), ...instructions);
  tx.feePayer = payerCanSign ? payer.publicKey : owner;
  tx.recentBlockhash = (await connection.getLatestBlockhash("confirmed")).blockhash;
  return tx;
}

async function serializeOwnerTransaction(owner: PublicKey, instructions: TransactionInstruction[]) {
  const tx = await ownerTransaction(owner, instructions);
  return tx.serialize({ requireAllSignatures: false, verifySignatures: false }).toString("base64");
}

function rawAmount(body: Record<string, unknown>, keys: string[], decimals: number, fallback: string) {
  for (const key of keys) {
    const value = body[key];
    if (value != null && value !== "") return parseUnits(value as any, decimals);
  }
  return parseUnits(fallback, decimals);
}

function assetFromBody(value: unknown, fallback: MarketAsset): MarketAsset {
  if (value === "base" || value === "quote") return value;
  return fallback;
}

async function maybeAddAta(
  instructions: TransactionInstruction[],
  owner: PublicKey,
  mint: PublicKey,
  tokenProgram: PublicKey
) {
  const ata = await ataInstructionIfMissing({ payer: owner, owner, mint, tokenProgram });
  if (ata.instruction) instructions.push(ata.instruction);
  return ata.address;
}

async function buildAddLiquidityTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  baseDepositAmount: bigint;
  quoteDepositAmount: bigint;
  minYlpAmount: bigint;
  payerCanSign?: boolean;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const instructions: TransactionInstruction[] = [];
  const ownerBase = await maybeAddAta(instructions, params.owner, m.baseMint, m.baseTokenProgram);
  const ownerQuote = await maybeAddAta(instructions, params.owner, m.quoteMint, m.quoteTokenProgram);
  const ownerYlp = await maybeAddAta(instructions, params.owner, m.ylpMint, TOKEN_2022_PROGRAM_ID);

  const ix = await program.methods
    .addLiquidity({
      baseDepositAmount: toBN(params.baseDepositAmount),
      quoteDepositAmount: toBN(params.quoteDepositAmount),
      minYlpAmount: toBN(params.minYlpAmount),
    })
    .accounts({
      market: m.market,
      futarchyAuthority: m.futarchyAuthority,
      owner: params.owner,
      baseMint: m.baseMint,
      quoteMint: m.quoteMint,
      ylpMint: m.ylpMint,
      baseReserveVault: m.baseReserveVault,
      quoteReserveVault: m.quoteReserveVault,
      ownerBaseAccount: ownerBase,
      ownerQuoteAccount: ownerQuote,
      ownerYlpAccount: ownerYlp,
      baseYieldAccount: deriveYieldAccount(m.market, params.owner, m.baseMint, "ylp"),
      quoteYieldAccount: deriveYieldAccount(m.market, params.owner, m.quoteMint, "ylp"),
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      eventAuthority: m.eventAuthority,
      program: PROGRAM_ID,
    })
    .instruction();
  instructions.push(ix);
  return ownerTransaction(params.owner, instructions, params.payerCanSign);
}

async function buildSwapTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  assetIn: MarketAsset;
  exactAssetIn: bigint;
  minAssetOut: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const inIsBase = params.assetIn === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerIn = await maybeAddAta(
    instructions,
    params.owner,
    inIsBase ? m.baseMint : m.quoteMint,
    inIsBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  const ownerOut = await maybeAddAta(
    instructions,
    params.owner,
    inIsBase ? m.quoteMint : m.baseMint,
    inIsBase ? m.quoteTokenProgram : m.baseTokenProgram
  );

  let builder = program.methods
    .swap({
      exactAssetIn: toBN(params.exactAssetIn),
      minAssetOut: toBN(params.minAssetOut),
    })
    .accounts({
      market: m.market,
      futarchyAuthority: m.futarchyAuthority,
      trader: params.owner,
      assetInMint: inIsBase ? m.baseMint : m.quoteMint,
      assetOutMint: inIsBase ? m.quoteMint : m.baseMint,
      reserveInVault: inIsBase ? m.baseReserveVault : m.quoteReserveVault,
      reserveOutVault: inIsBase ? m.quoteReserveVault : m.baseReserveVault,
      feeInVault: inIsBase ? m.baseFeeVault : m.quoteFeeVault,
      traderAssetInAccount: ownerIn,
      traderAssetOutAccount: ownerOut,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      eventAuthority: m.eventAuthority,
      program: PROGRAM_ID,
    });

  const refreshedMarket = await program.account.market.fetch(m.market);
  const baseHlpSupply = toBigInt(field(field(refreshedMarket, "baseHlpVault", "base_hlp_vault"), "hlpSupply", "hlp_supply"));
  const quoteHlpSupply = toBigInt(
    field(field(refreshedMarket, "quoteHlpVault", "quote_hlp_vault"), "hlpSupply", "hlp_supply")
  );
  const remainingAccounts = [];
  if (baseHlpSupply > 0n) {
    remainingAccounts.push(
      { pubkey: m.ylpMint, isWritable: true, isSigner: false },
      { pubkey: m.baseHlpYlpVault, isWritable: true, isSigner: false }
    );
  }
  if (quoteHlpSupply > 0n) {
    remainingAccounts.push(
      { pubkey: m.ylpMint, isWritable: true, isSigner: false },
      { pubkey: m.quoteHlpYlpVault, isWritable: true, isSigner: false }
    );
  }
  if (remainingAccounts.length > 0) builder = builder.remainingAccounts(remainingAccounts);
  instructions.push(await builder.instruction());
  return serializeOwnerTransaction(params.owner, instructions);
}

async function buildDepositCollateralTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  marketAsset: MarketAsset;
  depositAmount: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const isBase = params.marketAsset === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerAsset = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseMint : m.quoteMint,
    isBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  instructions.push(
    await program.methods
      .depositCollateral({
        depositAmount: toBN(params.depositAmount),
      })
      .accounts({
        market: m.market,
        owner: params.owner,
        assetMint: isBase ? m.baseMint : m.quoteMint,
        collateralVault: isBase ? m.baseCollateralVault : m.quoteCollateralVault,
        ownerAssetAccount: ownerAsset,
        marginPosition: deriveMarginPosition(m.market, params.owner),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: m.eventAuthority,
        program: PROGRAM_ID,
      })
      .instruction()
  );
  return serializeOwnerTransaction(params.owner, instructions);
}

async function buildBorrowTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  borrowAsset: MarketAsset;
  borrowAmount: bigint;
  minDebtAmountOut: bigint;
  minHealthBps: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const isBase = params.borrowAsset === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerDebt = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseMint : m.quoteMint,
    isBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  instructions.push(
    await program.methods
      .borrow({
        borrowAmount: toBN(params.borrowAmount),
        minDebtAmountOut: toBN(params.minDebtAmountOut),
        minHealthBps: toBN(params.minHealthBps),
      })
      .accounts({
        market: m.market,
        futarchyAuthority: m.futarchyAuthority,
        owner: params.owner,
        debtAssetMint: isBase ? m.baseMint : m.quoteMint,
        collateralAssetMint: isBase ? m.quoteMint : m.baseMint,
        reserveVault: isBase ? m.baseReserveVault : m.quoteReserveVault,
        ownerDebtAccount: ownerDebt,
        marginPosition: deriveMarginPosition(m.market, params.owner),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: m.eventAuthority,
        program: PROGRAM_ID,
      })
      .instruction()
  );
  return serializeOwnerTransaction(params.owner, instructions);
}

async function buildRepayTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  repayAsset: MarketAsset;
  repayAmount: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const isBase = params.repayAsset === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerDebt = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseMint : m.quoteMint,
    isBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  instructions.push(
    await program.methods
      .repay({
        repayAmount: toBN(params.repayAmount),
      })
      .accounts({
        market: m.market,
        owner: params.owner,
        debtAssetMint: isBase ? m.baseMint : m.quoteMint,
        reserveVault: isBase ? m.baseReserveVault : m.quoteReserveVault,
        ownerDebtAccount: ownerDebt,
        marginPosition: deriveMarginPosition(m.market, params.owner),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: m.eventAuthority,
        program: PROGRAM_ID,
      })
      .instruction()
  );
  return serializeOwnerTransaction(params.owner, instructions);
}

async function buildOpenHedgeTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  targetAsset: MarketAsset;
  depositAmount: bigint;
  minHlpAmount: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const isBase = params.targetAsset === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerTarget = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseMint : m.quoteMint,
    isBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  const ownerHlp = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseHlpMint : m.quoteHlpMint,
    TOKEN_2022_PROGRAM_ID
  );
  instructions.push(
    await program.methods
      .openHedge({
        depositAmount: toBN(params.depositAmount),
        minHlpAmount: toBN(params.minHlpAmount),
      })
      .accounts({
        market: m.market,
        futarchyAuthority: m.futarchyAuthority,
        owner: params.owner,
        baseMint: m.baseMint,
        quoteMint: m.quoteMint,
        ylpMint: m.ylpMint,
        targetHlpMint: isBase ? m.baseHlpMint : m.quoteHlpMint,
        baseReserveVault: m.baseReserveVault,
        quoteReserveVault: m.quoteReserveVault,
        ownerTargetAccount: ownerTarget,
        ownerHlpAccount: ownerHlp,
        hlpYlpAccount: isBase ? m.baseHlpYlpVault : m.quoteHlpYlpVault,
        targetYieldAccount: deriveYieldAccount(
          m.market,
          params.owner,
          isBase ? m.baseMint : m.quoteMint,
          "hlp"
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: m.eventAuthority,
        program: PROGRAM_ID,
      })
      .instruction()
  );
  return serializeOwnerTransaction(params.owner, instructions);
}

async function buildCloseHedgeTx(params: {
  owner: PublicKey;
  market: StoredMarket;
  targetAsset: MarketAsset;
  hlpAmount: bigint;
  minTargetAmountOut: bigint;
}) {
  const { program } = initializeRuntime();
  const m = marketFromStored(params.market);
  const isBase = params.targetAsset === "base";
  const instructions: TransactionInstruction[] = [];
  const ownerTarget = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseMint : m.quoteMint,
    isBase ? m.baseTokenProgram : m.quoteTokenProgram
  );
  const ownerHlp = await maybeAddAta(
    instructions,
    params.owner,
    isBase ? m.baseHlpMint : m.quoteHlpMint,
    TOKEN_2022_PROGRAM_ID
  );
  instructions.push(
    await program.methods
      .closeHedge({
        hlpAmount: toBN(params.hlpAmount),
        minTargetAmountOut: toBN(params.minTargetAmountOut),
      })
      .accounts({
        market: m.market,
        futarchyAuthority: m.futarchyAuthority,
        owner: params.owner,
        baseMint: m.baseMint,
        quoteMint: m.quoteMint,
        ylpMint: m.ylpMint,
        targetHlpMint: isBase ? m.baseHlpMint : m.quoteHlpMint,
        baseReserveVault: m.baseReserveVault,
        quoteReserveVault: m.quoteReserveVault,
        borrowedInterestVault: isBase ? m.quoteInterestVault : m.baseInterestVault,
        ownerTargetAccount: ownerTarget,
        ownerHlpAccount: ownerHlp,
        hlpYlpAccount: isBase ? m.baseHlpYlpVault : m.quoteHlpYlpVault,
        targetYieldAccount: deriveYieldAccount(
          m.market,
          params.owner,
          isBase ? m.baseMint : m.quoteMint,
          "hlp"
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: m.eventAuthority,
        program: PROGRAM_ID,
      })
      .instruction()
  );
  return serializeOwnerTransaction(params.owner, instructions);
}

async function userPositionsPayload(wallet: PublicKey, stored: StoredMarket) {
  const { program } = initializeRuntime();
  const market = new PublicKey(stored.market);
  const margin = deriveMarginPosition(market, wallet);
  const marginPosition = await program.account.marginPosition.fetchNullable(margin);
  const now = new Date().toISOString();
  const positions = [];
  if (marginPosition) {
    positions.push({
      id: 1,
      eventType: "margin_position",
      market: stored.market,
      owner: wallet.toBase58(),
      assetMint: null,
      txSig: "",
      slot: 0,
      instructionIndex: 0,
      instructionPath: "fork-state",
      timestamp: now,
      payload: {
        address: margin.toBase58(),
        baseCollateral: stringValue(field(marginPosition, "baseCollateral", "base_collateral")),
        quoteCollateral: stringValue(field(marginPosition, "quoteCollateral", "quote_collateral")),
        fixedBaseShares: stringValue(field(marginPosition, "fixedBaseShares", "fixed_base_shares")),
        fixedQuoteShares: stringValue(field(marginPosition, "fixedQuoteShares", "fixed_quote_shares")),
      },
    });
  }
  return positions;
}

async function fundWallet(body: Record<string, unknown>, stored: StoredMarket) {
  if (!ALLOW_PUBLIC_FUNDING) throw new Error("Public fork wallet funding is disabled");
  const owner = new PublicKey(String(body.wallet ?? body.owner ?? body.publicKey ?? ""));
  const sol = Number(body.sol ?? DEFAULT_SOL_FUNDING);
  const baseAmount = rawAmount(body, ["baseAmount", "baseUiAmount", "tokenAmount"], stored.baseDecimals, DEFAULT_TOKEN_FUNDING_UI);
  const quoteAmount = rawAmount(
    body,
    ["quoteAmount", "quoteUiAmount", "tokenAmount"],
    stored.quoteDecimals,
    DEFAULT_TOKEN_FUNDING_UI
  );
  const maxBaseAmount = parseUnits(MAX_TOKEN_FUNDING_UI, stored.baseDecimals);
  const maxQuoteAmount = parseUnits(MAX_TOKEN_FUNDING_UI, stored.quoteDecimals);
  if (!Number.isFinite(sol) || sol < 0 || sol > MAX_SOL_FUNDING) {
    throw new Error(`Fork SOL funding must be between 0 and ${MAX_SOL_FUNDING}`);
  }
  if (baseAmount > maxBaseAmount || quoteAmount > maxQuoteAmount) {
    throw new Error(`Fork token funding is capped at ${MAX_TOKEN_FUNDING_UI} UI units`);
  }
  await setLamports(owner, sol);
  await setTokenBalance(owner, new PublicKey(stored.baseMint), baseAmount, new PublicKey(stored.baseTokenProgram));
  await setTokenBalance(owner, new PublicKey(stored.quoteMint), quoteAmount, new PublicKey(stored.quoteTokenProgram));
  return {
    wallet: owner.toBase58(),
    sol,
    baseAmount: baseAmount.toString(),
    quoteAmount: quoteAmount.toString(),
    baseMint: stored.baseMint,
    quoteMint: stored.quoteMint,
  };
}

async function txResponse(
  name: string,
  owner: PublicKey,
  stored: StoredMarket,
  transaction: string,
  extra: Record<string, unknown> = {}
) {
  return {
    success: true,
    data: {
      action: name,
      owner: owner.toBase58(),
      market: stored.market,
      rpcUrl: PUBLIC_RPC_URL,
      transaction,
      ...extra,
    },
  };
}

export async function route(req: http.IncomingMessage, body: Record<string, unknown>) {
  const url = new URL(req.url ?? "/", "http://localhost");
  const path = url.pathname.replace(/\/$/, "") || "/";

  if (req.method === "GET" && path === "/health") {
    return {
      ok: true,
      rpcUrl: SURFPOOL_RPC_URL,
      publicRpcUrl: PUBLIC_RPC_URL,
      runtimeInitialized: Boolean(runtime),
      runtimeError,
    };
  }

  const stored = await bootstrap();

  if (req.method === "GET" && path === "/api/v2/fork/config") {
    return { success: true, data: forkConfigPayload(stored) };
  }

  if (req.method === "GET" && path === "/api/v2/markets") {
    return {
      success: true,
      data: {
        markets: [await marketPayload(stored)],
        pagination: { limit: 1, offset: 0, total: 1 },
      },
    };
  }

  if (req.method === "GET" && path === `/api/v2/markets/${stored.market}`) {
    return { success: true, data: await marketPayload(stored) };
  }

  if (req.method === "GET" && path === `/api/v2/markets/${stored.market}/swaps`) {
    return {
      success: true,
      data: { swaps: [], pagination: { limit: 100, offset: 0, total: 0 } },
    };
  }

  const userPositionsMatch = path.match(/^\/api\/v2\/users\/([^/]+)\/positions$/);
  if (req.method === "GET" && userPositionsMatch) {
    const wallet = new PublicKey(userPositionsMatch[1]);
    return {
      success: true,
      data: { positions: await userPositionsPayload(wallet, stored) },
    };
  }

  const userActivityMatch = path.match(/^\/api\/v2\/users\/([^/]+)\/activity$/);
  if (req.method === "GET" && userActivityMatch) {
    return {
      success: true,
      data: { activity: [], pagination: { limit: 100, offset: 0, total: 0 } },
    };
  }

  if (req.method !== "POST") {
    throw new Error(`Unsupported route: ${req.method} ${path}`);
  }

  if (path === "/api/v2/fork/fund-wallet") {
    return { success: true, data: await fundWallet(body, stored) };
  }

  const owner = new PublicKey(String(body.owner ?? body.wallet ?? body.publicKey ?? ""));

  if (path === "/api/v2/fork/tx/add-liquidity") {
    const transaction = (
      await buildAddLiquidityTx({
        owner,
        market: stored,
        baseDepositAmount: rawAmount(body, ["baseDepositAmount", "baseAmount"], stored.baseDecimals, "1"),
        quoteDepositAmount: rawAmount(body, ["quoteDepositAmount", "quoteAmount"], stored.quoteDecimals, "1"),
        minYlpAmount: rawAmount(body, ["minYlpAmount", "minBaseYlpAmount"], stored.baseDecimals, "0"),
      })
    ).serialize({ requireAllSignatures: false, verifySignatures: false }).toString("base64");
    return txResponse("add-liquidity", owner, stored, transaction);
  }

  if (path === "/api/v2/fork/tx/swap") {
    const assetIn = assetFromBody(body.assetIn, "base");
    const decimals = assetIn === "base" ? stored.baseDecimals : stored.quoteDecimals;
    const transaction = await buildSwapTx({
      owner,
      market: stored,
      assetIn,
      exactAssetIn: rawAmount(body, ["exactAssetIn", "amountIn", "amount"], decimals, "1"),
      minAssetOut: rawAmount(
        body,
        ["minAssetOut", "minAmountOut"],
        assetIn === "base" ? stored.quoteDecimals : stored.baseDecimals,
        "0"
      ),
    });
    return txResponse("swap", owner, stored, transaction, { assetIn });
  }

  if (path === "/api/v2/fork/tx/deposit-collateral") {
    const marketAsset = assetFromBody(body.marketAsset ?? body.asset, "base");
    const transaction = await buildDepositCollateralTx({
      owner,
      market: stored,
      marketAsset,
      depositAmount: rawAmount(
        body,
        ["depositAmount", "amount"],
        marketAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "1"
      ),
    });
    return txResponse("deposit-collateral", owner, stored, transaction, { marketAsset });
  }

  if (path === "/api/v2/fork/tx/borrow") {
    const borrowAsset = assetFromBody(body.borrowAsset ?? body.asset, "quote");
    const decimals = borrowAsset === "base" ? stored.baseDecimals : stored.quoteDecimals;
    const amount = rawAmount(body, ["borrowAmount", "amount"], decimals, "1");
    const minDebtAmountOut =
      body.minDebtAmountOut != null && body.minDebtAmountOut !== ""
        ? rawAmount(body, ["minDebtAmountOut"], decimals, "0")
        : amount;
    const transaction = await buildBorrowTx({
      owner,
      market: stored,
      borrowAsset,
      borrowAmount: amount,
      minDebtAmountOut,
      minHealthBps: BigInt(String(body.minHealthBps ?? "11000")),
    });
    return txResponse("borrow", owner, stored, transaction, { borrowAsset });
  }

  if (path === "/api/v2/fork/tx/repay") {
    const repayAsset = assetFromBody(body.repayAsset ?? body.asset, "quote");
    const transaction = await buildRepayTx({
      owner,
      market: stored,
      repayAsset,
      repayAmount: rawAmount(
        body,
        ["repayAmount", "amount"],
        repayAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "1"
      ),
    });
    return txResponse("repay", owner, stored, transaction, { repayAsset });
  }

  if (path === "/api/v2/fork/tx/open-hedge") {
    const targetAsset = assetFromBody(body.targetAsset ?? body.asset, "base");
    const transaction = await buildOpenHedgeTx({
      owner,
      market: stored,
      targetAsset,
      depositAmount: rawAmount(
        body,
        ["depositAmount", "amount"],
        targetAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "1"
      ),
      minHlpAmount: rawAmount(
        body,
        ["minHlpAmount"],
        targetAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "0"
      ),
    });
    return txResponse("open-hedge", owner, stored, transaction, { targetAsset });
  }

  if (path === "/api/v2/fork/tx/close-hedge") {
    const targetAsset = assetFromBody(body.targetAsset ?? body.asset, "base");
    const transaction = await buildCloseHedgeTx({
      owner,
      market: stored,
      targetAsset,
      hlpAmount: rawAmount(
        body,
        ["hlpAmount", "amount"],
        targetAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "1"
      ),
      minTargetAmountOut: rawAmount(
        body,
        ["minTargetAmountOut", "minAmountOut"],
        targetAsset === "base" ? stored.baseDecimals : stored.quoteDecimals,
        "0"
      ),
    });
    return txResponse("close-hedge", owner, stored, transaction, { targetAsset });
  }

  throw new Error(`Unsupported route: ${req.method} ${path}`);
}

export async function localE2E() {
  const stored = await bootstrap();
  const { payer, provider } = initializeRuntime();
  await fundWallet({ wallet: payer.publicKey.toBase58(), sol: DEFAULT_SOL_FUNDING }, stored);

  const addLiquidityTx = await buildAddLiquidityTx({
    owner: payer.publicKey,
    market: stored,
    baseDepositAmount: parseUnits("1", stored.baseDecimals),
    quoteDepositAmount: parseUnits("1", stored.quoteDecimals),
    minYlpAmount: 0n,
    payerCanSign: true,
  });
  addLiquidityTx.sign(payer);
  const addLiquiditySig = await provider.connection.sendRawTransaction(addLiquidityTx.serialize());
  await provider.connection.confirmTransaction(addLiquiditySig, "confirmed");

  const swapBase64 = await buildSwapTx({
    owner: payer.publicKey,
    market: stored,
    assetIn: "base",
    exactAssetIn: parseUnits("0.1", stored.baseDecimals),
    minAssetOut: 0n,
  });
  const swapTx = Transaction.from(Buffer.from(swapBase64, "base64"));
  swapTx.sign(payer);
  const swapSig = await provider.connection.sendRawTransaction(swapTx.serialize());
  await provider.connection.confirmTransaction(swapSig, "confirmed");

  return {
    ok: true,
    market: stored.market,
    addLiquiditySig,
    swapSig,
    config: forkConfigPayload(stored),
  };
}
