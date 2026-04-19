import * as fs from "fs";
import * as path from "path";
import { Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Program, AnchorProvider, Idl, Wallet } from "@coral-xyz/anchor";
import { LiteSVM } from "litesvm";
import { LiteSVMConnection } from "./litesvm-connection.js";

/**
 * Test setup configuration
 */
export interface TestSetupConfig {
  programPath?: string;
  idlPath?: string;
  initialBalance?: number;
}

/**
 * Test environment containing all necessary components for testing
 */
export interface TestEnvironment {
  svm: LiteSVM;
  connection: LiteSVMConnection;
  provider: AnchorProvider;
  program: Program;
  deployer: Keypair;
  payer: Keypair;
  programId: PublicKey;
}

/**
 * Initialize a test environment with LiteSVM
 * @param config Configuration for test setup
 * @returns Initialized test environment
 */
export async function initializeTestEnvironment(
  programId: PublicKey,
  config: TestSetupConfig = {}
): Promise<TestEnvironment> {
  const {
    programPath = path.join(__dirname, "../../target/deploy/omnipair.so"),
    idlPath = path.join(__dirname, "../../target/idl/omnipair.json"),
    initialBalance = 10 * LAMPORTS_PER_SOL,
  } = config;

  // Initialize LiteSVM
  const svm = new LiteSVM();

  // Load and add program
  if (!fs.existsSync(programPath)) {
    throw new Error(`Program file not found at ${programPath}. Please run 'anchor build' first.`);
  }

  svm.addProgramFromFile(programId, programPath);

  // Create connection wrapper
  const connection = new LiteSVMConnection(svm);

  // Create keypairs
  const deployer = Keypair.generate();
  const payer = Keypair.generate();

  // Airdrop SOL
  await connection.requestAirdrop(deployer.publicKey, initialBalance);
  await connection.requestAirdrop(payer.publicKey, initialBalance);

  // Load IDL
  if (!fs.existsSync(idlPath)) {
    throw new Error(`IDL file not found at ${idlPath}`);
  }

  const idl: Idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));

  // Create provider and program
  const wallet = new Wallet(payer);
  const provider = new AnchorProvider(connection as any, wallet as any, {});

  const program = new Program(idl as any, programId as any, provider as any);

  return {
    svm,
    connection,
    provider,
    program,
    deployer,
    payer,
    programId,
  };
}

/**
 * Find a program-derived address (PDA) for common seeds
 * @param seeds Seed strings to use for PDA derivation
 * @param programId Program ID
 * @returns [PDA PublicKey, bump seed]
 */
export function findPDA(seeds: string[], programId: PublicKey): [PublicKey, number] {
  const seedBuffers = seeds.map((s) => Buffer.from(s));
  return PublicKey.findProgramAddressSync(seedBuffers, programId);
}

/**
 * Create a new keypair and airdrop SOL to it
 * @param connection LiteSVM connection
 * @param amount Amount of SOL to airdrop
 * @returns Keypair with airdropped balance
 */
export async function createFundedKeypair(
  connection: LiteSVMConnection,
  amount: number = LAMPORTS_PER_SOL
): Promise<Keypair> {
  const keypair = Keypair.generate();
  await connection.requestAirdrop(keypair.publicKey, amount);
  return keypair;
}

/**
 * Get formatted balance in SOL
 * @param balance Balance in lamports
 * @returns Formatted balance string
 */
export function formatBalance(balance: number): string {
  return `${balance / LAMPORTS_PER_SOL} SOL`;
}

/**
 * Verify account existence and return account info
 * @param connection LiteSVM connection
 * @param publicKey Account public key
 * @returns Account info or null if not found
 */
export async function getAccountInfo(connection: LiteSVMConnection, publicKey: PublicKey) {
  return connection.getAccountInfo(publicKey);
}
