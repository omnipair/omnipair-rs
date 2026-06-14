import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import anchor from "@coral-xyz/anchor";
import { createMint, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
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

function deriveAddress(...seeds: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, OMNIPAIR_PROGRAM_ID)[0];
}

function marketConfig() {
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

describe("Omnipair Market LiteSVM", () => {
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

  it("initializes a market account", async () => {
    trackInstruction("initializeMarket", "initializes a market account");

    const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const [asset0Mint, asset1Mint] = orderedMints(mintA, mintB);
    const paramsHash = Buffer.alloc(32, 7);
    const [market] = PublicKey.findProgramAddressSync(
      [Buffer.from("market"), asset0Mint.toBuffer(), asset1Mint.toBuffer(), paramsHash],
      OMNIPAIR_PROGRAM_ID
    );
    const [eventAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      OMNIPAIR_PROGRAM_ID
    );
    const claim0Mint = await createMint(connection as any, payer, market, null, 6);
    const claim1Mint = await createMint(connection as any, payer, market, null, 6);
    const hedge0Mint = await createMint(connection as any, payer, market, null, 6);
    const hedge1Mint = await createMint(connection as any, payer, market, null, 6);
    const hedge0Vault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), claim0Mint.toBuffer());
    const hedge1Vault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), claim1Mint.toBuffer());
    const reserve0Vault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), asset0Mint.toBuffer());
    const reserve1Vault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), asset1Mint.toBuffer());
    const collateral0Vault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), asset0Mint.toBuffer());
    const collateral1Vault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), asset1Mint.toBuffer());
    const insurance0Vault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), asset0Mint.toBuffer());
    const insurance1Vault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), asset1Mint.toBuffer());
    const fee0Vault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), asset0Mint.toBuffer());
    const fee1Vault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), asset1Mint.toBuffer());
    const claim0StakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), claim0Mint.toBuffer());
    const claim1StakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), claim1Mint.toBuffer());

    await program.methods
      .initializeMarket({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config: marketConfig(),
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        asset0Mint,
        asset1Mint,
        market,
        claim0Mint,
        claim1Mint,
        hedge0Mint,
        hedge1Mint,
        hedge0Vault,
        hedge1Vault,
        reserve0Vault,
        reserve1Vault,
        collateral0Vault,
        collateral1Vault,
        insurance0Vault,
        insurance1Vault,
        fee0Vault,
        fee1Vault,
        claim0StakeVault,
        claim1StakeVault,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const marketAccount = await connection.getAccountInfo(market);
    expect(marketAccount).to.not.equal(null);
    expect(marketAccount.owner.toString()).to.equal(OMNIPAIR_PROGRAM_ID.toString());

    for (const vault of [reserve0Vault, reserve1Vault, hedge0Vault, hedge1Vault, claim0StakeVault, claim1StakeVault]) {
      const vaultAccount = await connection.getAccountInfo(vault);
      expect(vaultAccount).to.not.equal(null);
      expect(vaultAccount.owner.toString()).to.equal(TOKEN_PROGRAM_ID.toString());
    }
  });
});

after(() => {
  getCoverageReport();
});
