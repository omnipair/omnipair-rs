import { PublicKey } from "@solana/web3.js";

/** Default Omnipair program ID (mainnet) when env is not set */
const DEFAULT_PROGRAM_ID = "omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE";

function getProgramIdFromEnv(): string {
  if (typeof process === "undefined" || !process.env) return DEFAULT_PROGRAM_ID;
  return process.env.PROGRAM_ID ?? process.env.OMNIPAIR_PROGRAM_ID ?? DEFAULT_PROGRAM_ID;
}

/**
 * Omnipair program ID (mainnet/devnet).
 * Reads from env PROGRAM_ID or OMNIPAIR_PROGRAM_ID, falls back to mainnet default.
 */
export const PROGRAM_ID = new PublicKey(getProgramIdFromEnv());

/**
 * PDA seeds used by the program
 */
export const SEEDS = {
  PAIR: Buffer.from("gamm_pair"),
  MARKET_V2: Buffer.from("market_v2"),
  MARKET_RESERVE_VAULT: Buffer.from("market_reserve"),
  MARKET_COLLATERAL_VAULT: Buffer.from("market_collateral"),
  MARKET_FEE_VAULT: Buffer.from("market_fee"),
  MARKET_STAKE_VAULT: Buffer.from("market_stake"),
  STAKE_POSITION: Buffer.from("stake"),
  MARGIN_POSITION: Buffer.from("margin"),
  HEDGE_VAULT: Buffer.from("hedged"),
  HEDGE_POSITION: Buffer.from("hedge_position"),
  INSURANCE_RESERVE: Buffer.from("insurance"),
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
 * Derive V2 Market PDA address
 */
export function deriveMarketV2Address(
  asset0Mint: PublicKey,
  asset1Mint: PublicKey,
  paramsHash: Uint8Array | Buffer | number[]
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.MARKET_V2,
      asset0Mint.toBuffer(),
      asset1Mint.toBuffer(),
      normalizeParamsHash(paramsHash),
    ],
    PROGRAM_ID
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
    PROGRAM_ID
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
    PROGRAM_ID
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
    PROGRAM_ID
  );
}

/**
 * Derive market stake vault PDA address
 */
export function deriveMarketStakeVaultAddress(
  market: PublicKey,
  claimMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.MARKET_STAKE_VAULT, market.toBuffer(), claimMint.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive stake position PDA address
 */
export function deriveStakePositionAddress(
  market: PublicKey,
  owner: PublicKey,
  assetMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.STAKE_POSITION,
      market.toBuffer(),
      owner.toBuffer(),
      assetMint.toBuffer(),
    ],
    PROGRAM_ID
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
    PROGRAM_ID
  );
}

/**
 * Derive hedge vault PDA address
 */
export function deriveHedgeVaultAddress(
  market: PublicKey,
  claimMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.HEDGE_VAULT, market.toBuffer(), claimMint.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive hedge position PDA address
 */
export function deriveHedgePositionAddress(
  market: PublicKey,
  owner: PublicKey,
  assetMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      SEEDS.HEDGE_POSITION,
      market.toBuffer(),
      owner.toBuffer(),
      assetMint.toBuffer(),
    ],
    PROGRAM_ID
  );
}

/**
 * Derive insurance reserve PDA address
 */
export function deriveInsuranceReserveAddress(
  market: PublicKey,
  assetMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.INSURANCE_RESERVE, market.toBuffer(), assetMint.toBuffer()],
    PROGRAM_ID
  );
}
