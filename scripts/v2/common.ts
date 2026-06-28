import anchor from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  ExtensionType,
  NATIVE_MINT,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createInitializeMintInstruction,
  createInitializeTransferHookInstruction,
  createMint,
  getMintLen,
  getAccount,
  getMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import * as crypto from "crypto";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

export const NAD = new anchor.BN(1_000_000_000);
export const TOKEN_PROGRAMS = {
  token: TOKEN_PROGRAM_ID,
  token2022: TOKEN_2022_PROGRAM_ID,
} as const;
export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

export type StoredMint = {
  label: string;
  mint: string;
  decimals: number;
  tokenProgram: string;
  keypairPath: string;
  mintAuthority: string;
};

export type StoredMarket = {
  label: string;
  programId: string;
  market: string;
  paramsHash: string;
  baseMint: string;
  quoteMint: string;
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
  seededLiquidity?: boolean;
};

export type DevnetState = {
  network: string;
  mockMints: Record<string, StoredMint>;
  markets: Record<string, StoredMarket>;
};

export function configDir(): string {
  return (
    process.env.OMNIPAIR_V2_DEVNET_CONFIG_DIR ??
    path.join(os.homedir(), ".config", "omnipair", "v2-devnet")
  );
}

export function statePath(): string {
  return process.env.OMNIPAIR_V2_DEVNET_STATE ?? path.join(configDir(), "devnet-state.json");
}

export function ensureConfigDir(): void {
  fs.mkdirSync(configDir(), { recursive: true, mode: 0o700 });
}

export function readState(): DevnetState {
  ensureConfigDir();
  if (!fs.existsSync(statePath())) {
    return { network: "devnet", mockMints: {}, markets: {} };
  }
  return JSON.parse(fs.readFileSync(statePath(), "utf8")) as DevnetState;
}

export function writeState(state: DevnetState): void {
  ensureConfigDir();
  fs.writeFileSync(statePath(), `${JSON.stringify(state, null, 2)}\n`, { mode: 0o600 });
}

export function providerFromEnv(): anchor.AnchorProvider {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  provider.opts.commitment = "confirmed";
  provider.opts.preflightCommitment = "confirmed";
  provider.opts.skipPreflight = false;
  return provider;
}

export function payerFromProvider(provider: anchor.AnchorProvider): Keypair {
  const payer = (provider.wallet as any).payer as Keypair | undefined;
  if (!payer) {
    throw new Error("ANCHOR_WALLET must point to a local keypair wallet");
  }
  return payer;
}

export function keypairPath(label: string): string {
  const safeLabel = label.replace(/[^a-zA-Z0-9_-]/g, "-");
  return path.join(configDir(), `${safeLabel}.json`);
}

export function loadOrCreateKeypair(label: string): { keypair: Keypair; path: string; created: boolean } {
  ensureConfigDir();
  const filePath = keypairPath(label);
  if (fs.existsSync(filePath)) {
    const secret = JSON.parse(fs.readFileSync(filePath, "utf8")) as number[];
    return {
      keypair: Keypair.fromSecretKey(Uint8Array.from(secret)),
      path: filePath,
      created: false,
    };
  }

  const keypair = Keypair.generate();
  fs.writeFileSync(filePath, JSON.stringify(Array.from(keypair.secretKey)), { mode: 0o600 });
  return { keypair, path: filePath, created: true };
}

export async function tokenProgramForMint(
  connection: Connection,
  mint: PublicKey
): Promise<PublicKey> {
  const account = await connection.getAccountInfo(mint, "confirmed");
  if (!account) {
    throw new Error(`Mint account not found: ${mint.toBase58()}`);
  }
  return account.owner.equals(TOKEN_2022_PROGRAM_ID) ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
}

export async function mintDecimals(connection: Connection, mint: PublicKey): Promise<number> {
  const tokenProgram = await tokenProgramForMint(connection, mint);
  return (await getMint(connection, mint, "confirmed", tokenProgram)).decimals;
}

export async function createMintIfMissing(params: {
  connection: Connection;
  payer: Keypair;
  label: string;
  decimals: number;
  mintAuthority: PublicKey;
  tokenProgram?: PublicKey;
}): Promise<StoredMint> {
  const tokenProgram = params.tokenProgram ?? TOKEN_PROGRAM_ID;
  const { keypair, path: mintKeypairPath } = loadOrCreateKeypair(`mint-${params.label}`);
  const existing = await params.connection.getAccountInfo(keypair.publicKey, "confirmed");

  if (!existing) {
    await createMint(
      params.connection,
      params.payer,
      params.mintAuthority,
      null,
      params.decimals,
      keypair,
      undefined,
      tokenProgram
    );
  }

  return {
    label: params.label,
    mint: keypair.publicKey.toBase58(),
    decimals: params.decimals,
    tokenProgram: tokenProgram.toBase58(),
    keypairPath: mintKeypairPath,
    mintAuthority: params.mintAuthority.toBase58(),
  };
}

export async function createHookedLpMintIfMissing(params: {
  connection: Connection;
  payer: Keypair;
  label: string;
  decimals: number;
  mintAuthority: PublicKey;
  transferHookProgramId: PublicKey;
}): Promise<StoredMint> {
  const { keypair, path: mintKeypairPath } = loadOrCreateKeypair(`mint-${params.label}`);
  const existing = await params.connection.getAccountInfo(keypair.publicKey, "confirmed");

  if (!existing) {
    const mintLen = getMintLen([ExtensionType.TransferHook]);
    const lamports = await params.connection.getMinimumBalanceForRentExemption(mintLen);
    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: params.payer.publicKey,
        newAccountPubkey: keypair.publicKey,
        lamports,
        space: mintLen,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        keypair.publicKey,
        params.payer.publicKey,
        params.transferHookProgramId,
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
    await anchor.web3.sendAndConfirmTransaction(params.connection, transaction, [
      params.payer,
      keypair,
    ]);
  }

  return {
    label: params.label,
    mint: keypair.publicKey.toBase58(),
    decimals: params.decimals,
    tokenProgram: TOKEN_2022_PROGRAM_ID.toBase58(),
    keypairPath: mintKeypairPath,
    mintAuthority: params.mintAuthority.toBase58(),
  };
}

export async function getOrCreateAta(params: {
  connection: Connection;
  payer: Keypair;
  mint: PublicKey;
  owner: PublicKey;
  tokenProgram?: PublicKey;
  allowOwnerOffCurve?: boolean;
}) {
  const tokenProgram =
    params.tokenProgram ?? (await tokenProgramForMint(params.connection, params.mint));
  let lastError: unknown;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      return await getOrCreateAssociatedTokenAccount(
        params.connection,
        params.payer,
        params.mint,
        params.owner,
        params.allowOwnerOffCurve ?? false,
        "confirmed",
        undefined,
        tokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID
      );
    } catch (error) {
      lastError = error;
      if (!isTokenAccountConfirmationLag(error) || attempt === 4) {
        throw error;
      }
      await sleep(500 * (attempt + 1));
    }
  }
  throw lastError;
}

export async function mintMockTokens(params: {
  connection: Connection;
  payer: Keypair;
  mint: PublicKey;
  recipient: PublicKey;
  amount: bigint;
  tokenProgram?: PublicKey;
}) {
  const tokenProgram =
    params.tokenProgram ?? (await tokenProgramForMint(params.connection, params.mint));
  const recipientAccount = await getOrCreateAta({
    connection: params.connection,
    payer: params.payer,
    mint: params.mint,
    owner: params.recipient,
    tokenProgram,
  });
  const balanceBefore = await tokenAccountAmount(
    params.connection,
    recipientAccount.address,
    tokenProgram
  );

  const signature = await mintTo(
    params.connection,
    params.payer,
    params.mint,
    recipientAccount.address,
    params.payer,
    params.amount,
    [],
    undefined,
    tokenProgram
  );
  await waitForTokenBalanceAtLeast(
    params.connection,
    recipientAccount.address,
    balanceBefore + params.amount,
    tokenProgram
  );

  return { associatedTokenAccount: recipientAccount.address, signature };
}

function isTokenAccountConfirmationLag(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  return (
    error.name === "TokenAccountNotFoundError" ||
    error.constructor.name === "TokenAccountNotFoundError"
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function tokenAccountAmount(
  connection: Connection,
  tokenAccount: PublicKey,
  tokenProgram: PublicKey
): Promise<bigint> {
  try {
    return (await getAccount(connection, tokenAccount, "confirmed", tokenProgram)).amount;
  } catch (error) {
    if (!isTokenAccountConfirmationLag(error)) throw error;
    return 0n;
  }
}

async function waitForTokenBalanceAtLeast(
  connection: Connection,
  tokenAccount: PublicKey,
  expectedAmount: bigint,
  tokenProgram: PublicKey
): Promise<void> {
  let lastAmount = 0n;
  for (let attempt = 0; attempt < 8; attempt += 1) {
    lastAmount = await tokenAccountAmount(connection, tokenAccount, tokenProgram);
    if (lastAmount >= expectedAmount) return;
    await sleep(500 * (attempt + 1));
  }
  throw new Error(
    `Token balance confirmation timed out for ${tokenAccount.toBase58()}: ` +
      `${lastAmount.toString()} < ${expectedAmount.toString()}`
  );
}

export function parseUnits(value: string, decimals: number): bigint {
  const trimmed = value.trim();
  if (!/^\d+(\.\d+)?$/.test(trimmed)) {
    throw new Error(`Invalid decimal amount: ${value}`);
  }
  const [whole, fraction = ""] = trimmed.split(".");
  const normalizedFraction = fraction.padEnd(decimals, "0").slice(0, decimals);
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(normalizedFraction || "0");
}

export function bnFromUnits(value: bigint): anchor.BN {
  return new anchor.BN(value.toString());
}

export function publicKeyFromEnv(name: string, fallback?: PublicKey): PublicKey {
  const value = process.env[name];
  if (value) return new PublicKey(value);
  if (fallback) return fallback;
  throw new Error(`${name} is required`);
}

export function orderedMints(mintA: PublicKey, mintB: PublicKey): [PublicKey, PublicKey] {
  return Buffer.compare(mintA.toBuffer(), mintB.toBuffer()) < 0 ? [mintA, mintB] : [mintB, mintA];
}

export function paramsHashForMarket(label: string, baseMint: PublicKey, quoteMint: PublicKey): Buffer {
  const override = process.env.OMNIPAIR_V2_MARKET_PARAMS_HASH;
  if (override) {
    const bytes = Buffer.from(override.replace(/^0x/, ""), "hex");
    if (bytes.length !== 32) {
      throw new Error("OMNIPAIR_V2_MARKET_PARAMS_HASH must be 32 bytes of hex");
    }
    return bytes;
  }
  return crypto
    .createHash("sha256")
    .update(`omnipair-v2-final-devnet:${label}:${baseMint.toBase58()}:${quoteMint.toBase58()}`)
    .digest();
}

export function derivePda(programId: PublicKey, ...seeds: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

export function deriveTokenMetadataAddress(mint: PublicKey): PublicKey {
  return derivePda(
    TOKEN_METADATA_PROGRAM_ID,
    Buffer.from("metadata"),
    TOKEN_METADATA_PROGRAM_ID.toBuffer(),
    mint.toBuffer()
  );
}

export function deriveMarketAddresses(params: {
  programId: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
  paramsHash: Buffer;
}) {
  const market = derivePda(
    params.programId,
    Buffer.from("market_v2"),
    params.baseMint.toBuffer(),
    params.quoteMint.toBuffer(),
    params.paramsHash
  );
  return {
    market,
    eventAuthority: derivePda(params.programId, Buffer.from("__event_authority")),
    baseReserveVault: derivePda(params.programId, Buffer.from("market_reserve"), market.toBuffer(), params.baseMint.toBuffer()),
    quoteReserveVault: derivePda(params.programId, Buffer.from("market_reserve"), market.toBuffer(), params.quoteMint.toBuffer()),
    baseCollateralVault: derivePda(params.programId, Buffer.from("market_collateral"), market.toBuffer(), params.baseMint.toBuffer()),
    quoteCollateralVault: derivePda(params.programId, Buffer.from("market_collateral"), market.toBuffer(), params.quoteMint.toBuffer()),
    baseInsuranceVault: derivePda(params.programId, Buffer.from("insurance"), market.toBuffer(), params.baseMint.toBuffer()),
    quoteInsuranceVault: derivePda(params.programId, Buffer.from("insurance"), market.toBuffer(), params.quoteMint.toBuffer()),
    baseFeeVault: derivePda(params.programId, Buffer.from("market_fee"), market.toBuffer(), params.baseMint.toBuffer()),
    quoteFeeVault: derivePda(params.programId, Buffer.from("market_fee"), market.toBuffer(), params.quoteMint.toBuffer()),
    baseInterestVault: derivePda(params.programId, Buffer.from("market_interest"), market.toBuffer(), params.baseMint.toBuffer()),
    quoteInterestVault: derivePda(params.programId, Buffer.from("market_interest"), market.toBuffer(), params.quoteMint.toBuffer()),
  };
}

export function deriveFutarchyAuthorityAddress(programId: PublicKey): PublicKey {
  return derivePda(programId, Buffer.from("futarchy_authority"));
}

export function deriveProgramDataAddress(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [programId.toBuffer()],
    new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
  )[0];
}

export function deriveYieldAccountAddress(
  programId: PublicKey,
  market: PublicKey,
  owner: PublicKey,
  assetMint: PublicKey,
  tokenKind: "ylp" | "hlp"
): PublicKey {
  return derivePda(
    programId,
    Buffer.from("yield"),
    market.toBuffer(),
    owner.toBuffer(),
    assetMint.toBuffer(),
    Buffer.from([tokenKind === "ylp" ? 0 : 1])
  );
}

export function deriveHlpYlpVaultAddress(
  programId: PublicKey,
  market: PublicKey,
  targetHlpMint: PublicKey,
  ylpMint: PublicKey
): PublicKey {
  return derivePda(
    programId,
    Buffer.from("hlp_ylp_vault"),
    market.toBuffer(),
    targetHlpMint.toBuffer(),
    ylpMint.toBuffer()
  );
}

export function defaultMarketConfig() {
  const startTime = process.env.OMNIPAIR_V2_MARKET_START_TIME ?? "0";
  return {
    swapFeeBps: Number(process.env.OMNIPAIR_V2_SWAP_FEE_BPS ?? "30"),
    operatorFeeBps: Number(process.env.OMNIPAIR_V2_OPERATOR_FEE_BPS ?? "0"),
    protocolFeeBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_FEE_BPS ?? "0"),
    targetHlpLeverageBps: Number(process.env.OMNIPAIR_V2_TARGET_HLP_LEVERAGE_BPS ?? "20000"),
    settlementDivergenceBps: Number(process.env.OMNIPAIR_V2_SETTLEMENT_DIVERGENCE_BPS ?? "500"),
    emergencyExitHaircutBps: Number(process.env.OMNIPAIR_V2_EMERGENCY_EXIT_HAIRCUT_BPS ?? "250"),
    emaHalfLifeMs: new anchor.BN(process.env.OMNIPAIR_V2_EMA_HALF_LIFE_MS ?? "60000"),
    directionalEmaHalfLifeMs: new anchor.BN(process.env.OMNIPAIR_V2_DIRECTIONAL_EMA_HALF_LIFE_MS ?? "60000"),
    kEmaHalfLifeMs: new anchor.BN(process.env.OMNIPAIR_V2_K_EMA_HALF_LIFE_MS ?? "60000"),
    maxDailyBorrowBps: Number(process.env.OMNIPAIR_V2_MAX_DAILY_BORROW_BPS ?? "2000"),
    maxDailyWithdrawBps: Number(process.env.OMNIPAIR_V2_MAX_DAILY_WITHDRAW_BPS ?? "2000"),
    spotEmaDivergenceBps: Number(process.env.OMNIPAIR_V2_SPOT_EMA_DIVERGENCE_BPS ?? "1000"),
    kEmaDrawdownBps: Number(process.env.OMNIPAIR_V2_K_EMA_DRAWDOWN_BPS ?? "1000"),
    recognizedCollateralCapBps: Number(process.env.OMNIPAIR_V2_RECOGNIZED_COLLATERAL_CAP_BPS ?? "15000"),
    marketHealthMinBps: Number(process.env.OMNIPAIR_V2_MARKET_HEALTH_MIN_BPS ?? "11000"),
    liquidationAuctionDurationSlots: new anchor.BN(process.env.OMNIPAIR_V2_LIQUIDATION_AUCTION_DURATION_SLOTS ?? "1200"),
    liquidationAuctionStartIncentiveBps: Number(process.env.OMNIPAIR_V2_LIQUIDATION_AUCTION_START_INCENTIVE_BPS ?? "0"),
    hedgedLpEnabled: process.env.OMNIPAIR_V2_HEDGED_LP_ENABLED !== "0",
    startTime: new anchor.BN(startTime),
  };
}

export function defaultLpMetadata(kind: "ylp" | "baseHlp" | "quoteHlp") {
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

export function v2Program(idl: any, provider: anchor.AnchorProvider): any {
  const programId = process.env.OMNIPAIR_V2_PROGRAM_ID ?? idl.address;
  return new anchor.Program({ ...idl, address: programId } as any, provider as any);
}

export function explorerTx(signature: string): string {
  return `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
}

export async function tokenBalance(connection: Connection, tokenAccount: PublicKey): Promise<string> {
  try {
    return (await getAccount(connection, tokenAccount)).amount.toString();
  } catch (_) {
    return "0";
  }
}

export { PublicKey, SystemProgram, TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID };
export { NATIVE_MINT };
