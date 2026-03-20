import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import { Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Program, AnchorProvider, Idl, Wallet } from "@coral-xyz/anchor";
import { LiteSVM } from "litesvm";
import { LiteSVMConnection } from "./utils/litesvm-connection.js";
import { trackInstruction, getCoverageReport } from "./utils/instruction-coverage.js";
import { expect } from "chai";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const omnipairIdlPath = path.join(__dirname, "../target/idl/omnipair.json");
const omnipairIdlData = JSON.parse(fs.readFileSync(omnipairIdlPath, "utf-8")) as any;
// Create a minimal IDL without accounts to avoid parsing issues
const omnipairIdl = {
  ...omnipairIdlData,
  accounts: []  // Remove accounts that cause parsing issues
} as any;

describe("Omnipair Program - Basic Tests", () => {
  let svm;
  let connection;
  let provider;
  let program;
  let payer;

  const OMNIPAIR_PROGRAM_ID = new PublicKey("Bd9Uhf5S8yzfop8cG9oqRs6jVcLtu8B4cb2gvRmtbNzk");

  before(async () => {
    svm = new LiteSVM();

    const programPath = path.join(__dirname, "../target/deploy/omnipair.so");
    if (!fs.existsSync(programPath)) {
      throw new Error(`Program file not found at ${programPath}`);
    }

    svm.addProgramFromFile(OMNIPAIR_PROGRAM_ID, programPath);
    connection = new LiteSVMConnection(svm);

    payer = Keypair.generate();
    await connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL);

    const wallet = new Wallet(payer);
    provider = new AnchorProvider(connection as any, wallet as any, {});
    program = new Program(omnipairIdl as any, OMNIPAIR_PROGRAM_ID as any, provider as any);
  });

  it("should have initialized the program", async () => {
    expect(program).to.not.be.undefined;
    expect(program.programId.toString()).to.equal(OMNIPAIR_PROGRAM_ID.toString());
  });

  it("should have airdropped SOL to payer", async () => {
    const balance = await connection.getBalance(payer.publicKey);
    expect(balance).to.equal(10 * LAMPORTS_PER_SOL);
  });
});

// Display coverage report after basic tests
after(() => {
  getCoverageReport();
});
