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
