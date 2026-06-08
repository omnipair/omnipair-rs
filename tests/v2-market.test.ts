import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import anchor from "@coral-xyz/anchor";
import { createMint } from "@solana/spl-token";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { LiteSVM } from "litesvm";
import { LiteSVMConnection } from "./utils/litesvm-connection.js";
import { trackInstruction, getCoverageReport } from "./utils/instruction-coverage.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OMNIPAIR_PROGRAM_ID = new PublicKey("omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE");
const { AnchorProvider, BN, Program, Wallet } = anchor;
const NAD = new BN(1_000_000_000);

const omnipairIdlPath = path.join(__dirname, "../target/idl/omnipair.json");
const omnipairIdlData = JSON.parse(fs.readFileSync(omnipairIdlPath, "utf-8")) as any;
const omnipairIdl = {
  ...omnipairIdlData,
  accounts: [],
} as any;

function orderedMints(mintA: PublicKey, mintB: PublicKey): [PublicKey, PublicKey] {
  return Buffer.compare(mintA.toBuffer(), mintB.toBuffer()) < 0
    ? [mintA, mintB]
    : [mintB, mintA];
}

function marketConfigV2() {
  return {
    swapFeeBps: 30,
    operatorFeeBps: 1_000,
    bufferRatioBps: 2_000,
    feeRoutingKNad: NAD,
    emaHalfLifeMs: new BN(60_000),
    directionalEmaHalfLifeMs: new BN(60_000),
    kEmaHalfLifeMs: new BN(60_000),
    maxDailyBorrowBps: 2_000,
    maxDailyWithdrawBps: 2_000,
    spotEmaDivergenceBps: 1_000,
    recognizedCollateralCapBps: 10_000,
    marketHealthMinBps: 11_000,
    effectiveDebtWeightMinBps: 10_000,
    effectiveDebtGammaNad: NAD,
    softBorrowEnabled: false,
    hedgedLpEnabled: true,
    startTime: new BN(0),
  };
}

describe("Omnipair V2 Market LiteSVM", () => {
  let connection: LiteSVMConnection;
  let provider: any;
  let program: any;
  let payer: Keypair;

  before(async () => {
    const svm = new LiteSVM();
    const programPath = path.join(__dirname, "../target/deploy/omnipair.so");
    if (!fs.existsSync(programPath)) {
      throw new Error(`Program file not found at ${programPath}`);
    }

    svm.addProgramFromFile(OMNIPAIR_PROGRAM_ID, programPath);
    connection = new LiteSVMConnection(svm);

    payer = Keypair.generate();
    await connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL);

    provider = new AnchorProvider(connection as any, new Wallet(payer) as any, {});
    program = new Program(omnipairIdl as any, provider as any);
  });

  it("initializes a V2 market account", async () => {
    trackInstruction("v2InitializeMarket", "initializes a V2 market account");

    const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const [asset0Mint, asset1Mint] = orderedMints(mintA, mintB);
    const paramsHash = Buffer.alloc(32, 7);
    const [market] = PublicKey.findProgramAddressSync(
      [Buffer.from("market_v2"), asset0Mint.toBuffer(), asset1Mint.toBuffer(), paramsHash],
      OMNIPAIR_PROGRAM_ID
    );
    const [eventAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      OMNIPAIR_PROGRAM_ID
    );

    await program.methods
      .v2InitializeMarket({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config: marketConfigV2(),
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        asset0Mint,
        asset1Mint,
        market,
        claim0Mint: Keypair.generate().publicKey,
        claim1Mint: Keypair.generate().publicKey,
        hedge0Mint: Keypair.generate().publicKey,
        hedge1Mint: Keypair.generate().publicKey,
        hedge0Vault: Keypair.generate().publicKey,
        hedge1Vault: Keypair.generate().publicKey,
        reserve0Vault: Keypair.generate().publicKey,
        reserve1Vault: Keypair.generate().publicKey,
        collateral0Vault: Keypair.generate().publicKey,
        collateral1Vault: Keypair.generate().publicKey,
        insurance0Vault: Keypair.generate().publicKey,
        insurance1Vault: Keypair.generate().publicKey,
        fee0Vault: Keypair.generate().publicKey,
        fee1Vault: Keypair.generate().publicKey,
        claim0StakeVault: Keypair.generate().publicKey,
        claim1StakeVault: Keypair.generate().publicKey,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const marketAccount = await connection.getAccountInfo(market);
    expect(marketAccount).to.not.equal(null);
    expect(marketAccount.owner.toString()).to.equal(OMNIPAIR_PROGRAM_ID.toString());
  });
});

after(() => {
  getCoverageReport();
});
