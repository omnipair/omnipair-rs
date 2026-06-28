import { AccountMeta, PublicKey } from "@solana/web3.js";

/** Default Omnipair program ID (mainnet) when env is not set */
const DEFAULT_PROGRAM_ID = "omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE";
const DEFAULT_V2_PROGRAM_ID = "358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv";
const MPL_TOKEN_METADATA_PROGRAM_ID = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

function getProgramIdFromEnv(envNames: string[], fallback: string): string {
  if (typeof process === "undefined" || !process.env) return fallback;
  for (const envName of envNames) {
    const value = process.env[envName];
    if (value) return value;
  }
  return fallback;
}

/**
 * Omnipair V1 program ID (mainnet/devnet).
 * Reads from env PROGRAM_ID or OMNIPAIR_PROGRAM_ID, falls back to mainnet default.
 */
export const PROGRAM_ID = new PublicKey(
  getProgramIdFromEnv(["PROGRAM_ID", "OMNIPAIR_PROGRAM_ID"], DEFAULT_PROGRAM_ID)
);

export const OMNIPAIR_PROGRAM_ID = PROGRAM_ID;

/**
 * Omnipair V2 program ID.
 * Reads from env OMNIPAIR_V2_PROGRAM_ID or PROGRAM_ID_V2, falls back to V2 default.
 */
export const OMNIPAIR_V2_PROGRAM_ID = new PublicKey(
  getProgramIdFromEnv(["OMNIPAIR_V2_PROGRAM_ID", "PROGRAM_ID_V2"], DEFAULT_V2_PROGRAM_ID)
);

export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(MPL_TOKEN_METADATA_PROGRAM_ID);

/**
 * PDA seeds used by the program
 */
export const SEEDS = {
  PAIR: Buffer.from("gamm_pair"),
  MARKET_V2: Buffer.from("market_v2"),
  MARKET_RESERVE_VAULT: Buffer.from("market_reserve"),
  MARKET_COLLATERAL_VAULT: Buffer.from("market_collateral"),
  MARKET_FEE_VAULT: Buffer.from("market_fee"),
  MARKET_INTEREST_VAULT: Buffer.from("market_interest"),
  MARGIN_POSITION: Buffer.from("margin"),
  YIELD_ACCOUNT: Buffer.from("yield"),
  HLP_YLP_VAULT: Buffer.from("hlp_ylp_vault"),
  INSURANCE: Buffer.from("insurance"),
  LIQUIDATION_AUCTION: Buffer.from("liquidation_auction"),
  USER_POSITION: Buffer.from("gamm_position"),
  FUTARCHY_AUTHORITY: Buffer.from("futarchy_authority"),
  RESERVE_VAULT: Buffer.from("reserve_vault"),
  COLLATERAL_VAULT: Buffer.from("collateral_vault"),
  METADATA: Buffer.from("metadata"),
} as const;

function normalizeParamsHash(paramsHash: Uint8Array | Buffer | number[]): Buffer {
  const hash = Buffer.from(paramsHash);
  if (hash.length !== 32) {
    throw new Error(`paramsHash must be 32 bytes, got ${hash.length}`);
  }
  return hash;
}

/**
 * Derive Pair PDA address
 */
export function derivePairAddress(
  token0: PublicKey,
  token1: PublicKey,
  paramsHash: Uint8Array | Buffer | number[]
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.PAIR, token0.toBuffer(), token1.toBuffer(), normalizeParamsHash(paramsHash)],
    PROGRAM_ID
  );
}

/**
 * Derive User Position PDA address
 */
export function deriveUserPositionAddress(
  pair: PublicKey,
  user: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.USER_POSITION, pair.toBuffer(), user.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive Futarchy Authority PDA address
 */
export function deriveFutarchyAuthorityAddress(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([SEEDS.FUTARCHY_AUTHORITY], PROGRAM_ID);
}

/**
 * Derive V2 Futarchy Authority PDA address.
 */
export function deriveFutarchyAuthorityV2Address(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([SEEDS.FUTARCHY_AUTHORITY], OMNIPAIR_V2_PROGRAM_ID);
}

/**
 * Derive Reserve Vault PDA address
 */
export function deriveReserveVaultAddress(
  pair: PublicKey,
  reserveMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.RESERVE_VAULT, pair.toBuffer(), reserveMint.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive Collateral Vault PDA address
 */
export function deriveCollateralVaultAddress(
  pair: PublicKey,
  collateralMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.COLLATERAL_VAULT, pair.toBuffer(), collateralMint.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive V2 market PDA address
 */
export function deriveMarketAddress(
  baseMint: PublicKey,
  quoteMint: PublicKey,
  paramsHash: Uint8Array | Buffer | number[]
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.MARKET_V2,
      baseMint.toBuffer(),
      quoteMint.toBuffer(),
      normalizeParamsHash(paramsHash),
    ],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

export const deriveMarketV2Address = deriveMarketAddress;

/**
 * Derive a Metaplex Token Metadata PDA for a mint.
 */
export function deriveTokenMetadataAddress(mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.METADATA, TOKEN_METADATA_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    TOKEN_METADATA_PROGRAM_ID
  );
}

/**
 * Derive market reserve vault PDA address
 */
export function deriveMarketReserveVaultAddress(
  market: PublicKey,
  reserveMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARKET_RESERVE_VAULT, market.toBuffer(), reserveMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive market collateral vault PDA address
 */
export function deriveMarketCollateralVaultAddress(
  market: PublicKey,
  collateralMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARKET_COLLATERAL_VAULT, market.toBuffer(), collateralMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive market fee vault PDA address
 */
export function deriveMarketFeeVaultAddress(
  market: PublicKey,
  feeMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARKET_FEE_VAULT, market.toBuffer(), feeMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive market interest vault PDA address
 */
export function deriveMarketInterestVaultAddress(
  market: PublicKey,
  interestMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARKET_INTEREST_VAULT, market.toBuffer(), interestMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive margin position PDA address
 */
export function deriveMarginPositionAddress(
  market: PublicKey,
  owner: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARGIN_POSITION, market.toBuffer(), owner.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive liquidation auction PDA address for a market margin position and debt mint.
 */
export function deriveLiquidationAuctionAddress(
  market: PublicKey,
  marginPosition: PublicKey,
  debtMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.LIQUIDATION_AUCTION,
      market.toBuffer(),
      marginPosition.toBuffer(),
      debtMint.toBuffer(),
    ],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

export type YieldTokenKind = "ylp" | "hlp" | 0 | 1;

function yieldTokenKindCode(tokenKind: YieldTokenKind): number {
  if (tokenKind === "ylp" || tokenKind === 0) return 0;
  if (tokenKind === "hlp" || tokenKind === 1) return 1;
  throw new Error(`Unsupported yield token kind: ${tokenKind}`);
}

/**
 * Derive yield-account PDA address for yLP/hLP revenue checkpoints.
 */
export function deriveYieldAccountAddress(
  market: PublicKey,
  owner: PublicKey,
  assetMint: PublicKey,
  tokenKind: YieldTokenKind
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.YIELD_ACCOUNT,
      market.toBuffer(),
      owner.toBuffer(),
      assetMint.toBuffer(),
      Buffer.from([yieldTokenKindCode(tokenKind)]),
    ],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

export interface YieldTransferHookAccountsArgs {
  lpMint: PublicKey;
  market: PublicKey;
  sourceOwner: PublicKey;
  destinationOwner: PublicKey;
  assetMint: PublicKey;
  tokenKind: YieldTokenKind;
}

export const TRANSFER_HOOK_EXECUTE_DISCRIMINATOR = Buffer.from([
  105, 37, 101, 197, 75, 251, 102, 26,
]);

const TRANSFER_HOOK_EXTRA_ACCOUNT_META_SIZE = 35;
const TRANSFER_HOOK_TOKEN_ACCOUNT_OWNER_OFFSET = 32;
const TRANSFER_HOOK_SOURCE_ACCOUNT_INDEX = 0;
const TRANSFER_HOOK_DESTINATION_ACCOUNT_INDEX = 2;
const TRANSFER_HOOK_MARKET_ACCOUNT_INDEX = 5;
const TRANSFER_HOOK_ASSET_MINT_ACCOUNT_INDEX = 6;
const TRANSFER_HOOK_QUOTE_ASSET_MINT_ACCOUNT_INDEX = 7;

type TransferHookSeed =
  | { kind: "literal"; bytes: Uint8Array | Buffer | number[] }
  | { kind: "instructionData"; index: number; length: number }
  | { kind: "accountKey"; index: number }
  | { kind: "accountData"; accountIndex: number; dataIndex: number; length: number };

interface TransferHookValidationMeta {
  discriminator: number;
  addressConfig: Buffer;
  isSigner: boolean;
  isWritable: boolean;
}

export interface YieldTransferHookValidationArgs {
  market: PublicKey;
  assetMint: PublicKey;
  tokenKind: YieldTokenKind;
}

export interface YlpTransferHookValidationArgs {
  market: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
}

/**
 * Derive the standard Token-2022 transfer-hook validation PDA for a V2 LP mint.
 */
export function deriveYieldTransferHookValidationAddress(lpMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("extra-account-metas"), lpMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

function assertU8(value: number, label: string): void {
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    throw new Error(`${label} must fit in a u8, got ${value}`);
  }
}

function packTransferHookSeedConfig(seeds: TransferHookSeed[]): Buffer {
  const config = Buffer.alloc(32);
  let offset = 0;
  const writeByte = (value: number, label: string) => {
    assertU8(value, label);
    if (offset >= config.length) {
      throw new Error("transfer-hook seed config exceeds 32 bytes");
    }
    config[offset] = value;
    offset += 1;
  };

  for (const seed of seeds) {
    switch (seed.kind) {
      case "literal": {
        const bytes = Buffer.from(seed.bytes);
        assertU8(bytes.length, "literal seed length");
        writeByte(1, "literal seed discriminator");
        writeByte(bytes.length, "literal seed length");
        if (offset + bytes.length > config.length) {
          throw new Error("transfer-hook seed config exceeds 32 bytes");
        }
        bytes.copy(config, offset);
        offset += bytes.length;
        break;
      }
      case "instructionData":
        writeByte(2, "instruction-data seed discriminator");
        writeByte(seed.index, "instruction-data seed index");
        writeByte(seed.length, "instruction-data seed length");
        break;
      case "accountKey":
        writeByte(3, "account-key seed discriminator");
        writeByte(seed.index, "account-key seed index");
        break;
      case "accountData":
        writeByte(4, "account-data seed discriminator");
        writeByte(seed.accountIndex, "account-data seed account index");
        writeByte(seed.dataIndex, "account-data seed data index");
        writeByte(seed.length, "account-data seed length");
        break;
    }
  }

  return config;
}

function staticTransferHookMeta(meta: AccountMeta): TransferHookValidationMeta {
  return {
    discriminator: 0,
    addressConfig: meta.pubkey.toBuffer(),
    isSigner: meta.isSigner,
    isWritable: meta.isWritable,
  };
}

function pdaTransferHookMeta(
  seeds: TransferHookSeed[],
  isWritable: boolean
): TransferHookValidationMeta {
  return {
    discriminator: 1,
    addressConfig: packTransferHookSeedConfig(seeds),
    isSigner: false,
    isWritable,
  };
}

function yieldAccountTransferHookSeeds(
  ownerTokenAccountIndex: number,
  tokenKind: YieldTokenKind,
  assetMintAccountIndex = TRANSFER_HOOK_ASSET_MINT_ACCOUNT_INDEX
): TransferHookSeed[] {
  return [
    { kind: "literal", bytes: SEEDS.YIELD_ACCOUNT },
    { kind: "accountKey", index: TRANSFER_HOOK_MARKET_ACCOUNT_INDEX },
    {
      kind: "accountData",
      accountIndex: ownerTokenAccountIndex,
      dataIndex: TRANSFER_HOOK_TOKEN_ACCOUNT_OWNER_OFFSET,
      length: 32,
    },
    { kind: "accountKey", index: assetMintAccountIndex },
    { kind: "literal", bytes: [yieldTokenKindCode(tokenKind)] },
  ];
}

function encodeTransferHookValidationAccountData(extraMetas: TransferHookValidationMeta[]): Buffer {
  const podSliceLength = 4 + extraMetas.length * TRANSFER_HOOK_EXTRA_ACCOUNT_META_SIZE;
  const data = Buffer.alloc(8 + 4 + podSliceLength);
  TRANSFER_HOOK_EXECUTE_DISCRIMINATOR.copy(data, 0);
  data.writeUInt32LE(podSliceLength, 8);
  data.writeUInt32LE(extraMetas.length, 12);

  let offset = 16;
  for (const meta of extraMetas) {
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

/**
 * Encode a static Token-2022 transfer-hook validation account for Execute.
 *
 * The returned bytes use the SPL TLV shape expected by Token-2022:
 * execute discriminator, pod-slice length, count, then static account metas.
 */
export function buildYieldTransferHookValidationAccountData(extraMetas: AccountMeta[]): Buffer {
  return encodeTransferHookValidationAccountData(extraMetas.map(staticTransferHookMeta));
}

/**
 * Encode the production Token-2022 transfer-hook validation account for V2
 * yLP/hLP mints.
 *
 * The validation state resolves static market/asset-mint accounts and derives
 * source/destination YieldAccount PDAs from the source and destination token
 * account owners observed by Token-2022 during Execute.
 */
export function buildYieldTransferHookYieldValidationAccountData({
  market,
  assetMint,
  tokenKind,
}: YieldTransferHookValidationArgs): Buffer {
  return encodeTransferHookValidationAccountData([
    staticTransferHookMeta({ pubkey: market, isSigner: false, isWritable: false }),
    staticTransferHookMeta({ pubkey: assetMint, isSigner: false, isWritable: false }),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(TRANSFER_HOOK_SOURCE_ACCOUNT_INDEX, tokenKind),
      true
    ),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(TRANSFER_HOOK_DESTINATION_ACCOUNT_INDEX, tokenKind),
      true
    ),
  ]);
}

export function buildYlpTransferHookValidationAccountData({
  market,
  baseMint,
  quoteMint,
}: YlpTransferHookValidationArgs): Buffer {
  return encodeTransferHookValidationAccountData([
    staticTransferHookMeta({ pubkey: market, isSigner: false, isWritable: false }),
    staticTransferHookMeta({ pubkey: baseMint, isSigner: false, isWritable: false }),
    staticTransferHookMeta({ pubkey: quoteMint, isSigner: false, isWritable: false }),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(TRANSFER_HOOK_SOURCE_ACCOUNT_INDEX, "ylp"),
      true
    ),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(TRANSFER_HOOK_DESTINATION_ACCOUNT_INDEX, "ylp"),
      true
    ),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(
        TRANSFER_HOOK_SOURCE_ACCOUNT_INDEX,
        "ylp",
        TRANSFER_HOOK_QUOTE_ASSET_MINT_ACCOUNT_INDEX
      ),
      true
    ),
    pdaTransferHookMeta(
      yieldAccountTransferHookSeeds(
        TRANSFER_HOOK_DESTINATION_ACCOUNT_INDEX,
        "ylp",
        TRANSFER_HOOK_QUOTE_ASSET_MINT_ACCOUNT_INDEX
      ),
      true
    ),
  ]);
}

/**
 * Build V2 yLP/hLP transfer-hook extra account metas.
 *
 * Token-2022 passes the source token account, LP mint, destination token
 * account, and transfer authority as the base hook accounts. Omnipair V2 needs
 * the market, underlying asset mint, canonical source/destination yield
 * accounts, hook program, and standard validation PDA as extra metas so the
 * hook can checkpoint revenue before the balance move is finalized.
 */
export function buildYieldTransferHookAccountMetas({
  lpMint,
  market,
  sourceOwner,
  destinationOwner,
  assetMint,
  tokenKind,
}: YieldTransferHookAccountsArgs): AccountMeta[] {
  const sourceYieldAccount = deriveYieldAccountAddress(
    market,
    sourceOwner,
    assetMint,
    tokenKind
  )[0];
  const destinationYieldAccount = deriveYieldAccountAddress(
    market,
    destinationOwner,
    assetMint,
    tokenKind
  )[0];
  const validationAccount = deriveYieldTransferHookValidationAddress(lpMint)[0];

  return [
    { pubkey: market, isSigner: false, isWritable: false },
    { pubkey: assetMint, isSigner: false, isWritable: false },
    { pubkey: sourceYieldAccount, isSigner: false, isWritable: true },
    { pubkey: destinationYieldAccount, isSigner: false, isWritable: true },
    { pubkey: OMNIPAIR_V2_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: validationAccount, isSigner: false, isWritable: false },
  ];
}

export function buildYlpTransferHookAccountMetas({
  lpMint,
  market,
  sourceOwner,
  destinationOwner,
  baseMint,
  quoteMint,
}: Omit<YieldTransferHookAccountsArgs, "assetMint" | "tokenKind"> & {
  baseMint: PublicKey;
  quoteMint: PublicKey;
}): AccountMeta[] {
  const sourceBaseYieldAccount = deriveYieldAccountAddress(
    market,
    sourceOwner,
    baseMint,
    "ylp"
  )[0];
  const destinationBaseYieldAccount = deriveYieldAccountAddress(
    market,
    destinationOwner,
    baseMint,
    "ylp"
  )[0];
  const sourceQuoteYieldAccount = deriveYieldAccountAddress(
    market,
    sourceOwner,
    quoteMint,
    "ylp"
  )[0];
  const destinationQuoteYieldAccount = deriveYieldAccountAddress(
    market,
    destinationOwner,
    quoteMint,
    "ylp"
  )[0];
  const validationAccount = deriveYieldTransferHookValidationAddress(lpMint)[0];

  return [
    { pubkey: market, isSigner: false, isWritable: false },
    { pubkey: baseMint, isSigner: false, isWritable: false },
    { pubkey: quoteMint, isSigner: false, isWritable: false },
    { pubkey: sourceBaseYieldAccount, isSigner: false, isWritable: true },
    { pubkey: destinationBaseYieldAccount, isSigner: false, isWritable: true },
    { pubkey: sourceQuoteYieldAccount, isSigner: false, isWritable: true },
    { pubkey: destinationQuoteYieldAccount, isSigner: false, isWritable: true },
    { pubkey: OMNIPAIR_V2_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: validationAccount, isSigner: false, isWritable: false },
  ];
}

/**
 * Derive canonical aggregate hLP vault yLP token-account PDA.
 */
export function deriveHlpYlpVaultAddress(
  market: PublicKey,
  targetHlpMint: PublicKey,
  ylpMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.HLP_YLP_VAULT,
      market.toBuffer(),
      targetHlpMint.toBuffer(),
      ylpMint.toBuffer(),
    ],
    OMNIPAIR_V2_PROGRAM_ID
  );
}

/**
 * Derive insurance PDA address
 */
export function deriveInsuranceAddress(
  market: PublicKey,
  assetMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.INSURANCE, market.toBuffer(), assetMint.toBuffer()],
    OMNIPAIR_V2_PROGRAM_ID
  );
}
