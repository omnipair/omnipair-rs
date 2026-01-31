import { PublicKey } from "@solana/web3.js";

/**
 * Omnipair program ID (mainnet/devnet)
 */
export const PROGRAM_ID = new PublicKey(
  "omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb"
);

/**
 * Development deployer address
 */
export const DEPLOYER_DEV = new PublicKey(
  "C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds"
);

/**
 * Production deployer address
 */
export const DEPLOYER_PROD = new PublicKey(
  "8tF4uYMBXqGhCUGRZL3AmPqRzbX8JJ1TpYnY3uJKN4kt"
);

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
