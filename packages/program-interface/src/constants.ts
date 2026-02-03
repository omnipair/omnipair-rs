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
  USER_POSITION: Buffer.from("user_position"),
  FUTARCHY_AUTHORITY: Buffer.from("futarchy_authority"),
  RATE_MODEL: Buffer.from("rate_model"),
} as const;

/**
 * Derive Pair PDA address
 */
export function derivePairAddress(
  token0: PublicKey,
  token1: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.PAIR, token0.toBuffer(), token1.toBuffer()],
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
