import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import anchor from "@coral-xyz/anchor";
import {
  createAccount,
  createMint,
  getAccount,
  mintTo,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  ComputeBudgetProgram,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
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

async function expectRejects(action) {
  let rejected = false;
  try {
    await action();
  } catch (_) {
    rejected = true;
  }
  expect(rejected).to.equal(true);
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
    recognizedCollateralCapBps: 15_000,
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

  async function initializeMarketFixture() {
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
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .signers([payer])
      .rpc();

    return {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      claim1Mint,
      hedge0Mint,
      hedge1Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      collateral0Vault,
      collateral1Vault,
      insurance0Vault,
      insurance1Vault,
      fee0Vault,
      fee1Vault,
      hedge0Vault,
      hedge1Vault,
      claim0StakeVault,
      claim1StakeVault,
      eventAuthority,
    };
  }

  async function depositReserveSide(
    fixture,
    marketSideIndex,
    assetMint,
    claimMint,
    reserveVault,
    ownerAssetAccount,
    ownerClaimAccount,
    depositAmount = 1_000_000,
    minClaimAmount = 800_000,
    maxBufferAmount = 200_000
  ) {
    const stakePosition = deriveAddress(
      Buffer.from("stake"),
      fixture.market.toBuffer(),
      payer.publicKey.toBuffer(),
      assetMint.toBuffer()
    );

    await program.methods
      .depositReserve({
        marketSideIndex,
        depositAmount: new BN(depositAmount),
        minClaimAmount: new BN(minClaimAmount),
        maxBufferAmount: new BN(maxBufferAmount),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint,
        claimMint,
        reserveVault,
        ownerAssetAccount,
        ownerClaimAccount,
        stakePosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    return stakePosition;
  }

  async function fundTwoSidedMarket() {
    const fixture = await initializeMarketFixture();
    const ownerAsset0Account = await createAccount(
      connection as any,
      payer,
      fixture.asset0Mint,
      payer.publicKey
    );
    const ownerAsset1Account = await createAccount(
      connection as any,
      payer,
      fixture.asset1Mint,
      payer.publicKey
    );
    const ownerClaim0Account = await createAccount(
      connection as any,
      payer,
      fixture.claim0Mint,
      payer.publicKey
    );
    const ownerClaim1Account = await createAccount(
      connection as any,
      payer,
      fixture.claim1Mint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.asset0Mint,
      ownerAsset0Account,
      payer,
      2_000_000
    );
    await mintTo(
      connection as any,
      payer,
      fixture.asset1Mint,
      ownerAsset1Account,
      payer,
      2_000_000
    );

    const stake0Position = await depositReserveSide(
      fixture,
      0,
      fixture.asset0Mint,
      fixture.claim0Mint,
      fixture.reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account
    );
    const stake1Position = await depositReserveSide(
      fixture,
      1,
      fixture.asset1Mint,
      fixture.claim1Mint,
      fixture.reserve1Vault,
      ownerAsset1Account,
      ownerClaim1Account
    );

    return {
      ...fixture,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      ownerClaim1Account,
      stake0Position,
      stake1Position,
    };
  }

  async function fundTinyRoundingMarket() {
    const fixture = await initializeMarketFixture();
    const ownerAsset0Account = await createAccount(
      connection as any,
      payer,
      fixture.asset0Mint,
      payer.publicKey
    );
    const ownerAsset1Account = await createAccount(
      connection as any,
      payer,
      fixture.asset1Mint,
      payer.publicKey
    );
    const ownerClaim0Account = await createAccount(
      connection as any,
      payer,
      fixture.claim0Mint,
      payer.publicKey
    );
    const ownerClaim1Account = await createAccount(
      connection as any,
      payer,
      fixture.claim1Mint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.asset0Mint,
      ownerAsset0Account,
      payer,
      100
    );
    await mintTo(
      connection as any,
      payer,
      fixture.asset1Mint,
      ownerAsset1Account,
      payer,
      100
    );

    const stake0Position = await depositReserveSide(
      fixture,
      0,
      fixture.asset0Mint,
      fixture.claim0Mint,
      fixture.reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      6,
      4,
      2
    );
    const stake1Position = await depositReserveSide(
      fixture,
      1,
      fixture.asset1Mint,
      fixture.claim1Mint,
      fixture.reserve1Vault,
      ownerAsset1Account,
      ownerClaim1Account,
      6,
      4,
      2
    );

    return {
      ...fixture,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      ownerClaim1Account,
      stake0Position,
      stake1Position,
    };
  }

  it("initializes a market account", async () => {
    trackInstruction("initializeMarket", "initializes a market account");

    const {
      market,
      reserve0Vault,
      reserve1Vault,
      hedge0Vault,
      hedge1Vault,
      claim0StakeVault,
      claim1StakeVault,
    } = await initializeMarketFixture();

    const marketAccount = await connection.getAccountInfo(market);
    expect(marketAccount).to.not.equal(null);
    expect(marketAccount.owner.toString()).to.equal(OMNIPAIR_PROGRAM_ID.toString());

    for (const vault of [reserve0Vault, reserve1Vault, hedge0Vault, hedge1Vault, claim0StakeVault, claim1StakeVault]) {
      const vaultAccount = await connection.getAccountInfo(vault);
      expect(vaultAccount).to.not.equal(null);
      expect(vaultAccount.owner.toString()).to.equal(TOKEN_PROGRAM_ID.toString());
    }
  });

  it("updates market config and enforces reduce-only mode", async () => {
    trackInstruction("updateMarketConfig", "updates market buffer ratio");
    trackInstruction("setMarketReduceOnly", "blocks risk-increasing reserve deposits");

    const fixture = await initializeMarketFixture();
    const ownerAsset0Account = await createAccount(
      connection as any,
      payer,
      fixture.asset0Mint,
      payer.publicKey
    );
    const ownerClaim0Account = await createAccount(
      connection as any,
      payer,
      fixture.claim0Mint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.asset0Mint,
      ownerAsset0Account,
      payer,
      2_000_000
    );

    const config = marketConfig();
    config.bufferRatioBps = 1_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market: fixture.market,
        operator: payer.publicKey,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const stake0Position = await depositReserveSide(
      fixture,
      0,
      fixture.asset0Mint,
      fixture.claim0Mint,
      fixture.reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      1_000_000,
      900_000,
      100_000
    );

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(900_000)
    );

    await program.methods
      .setMarketReduceOnly({ reduceOnly: true })
      .accounts({
        market: fixture.market,
        operator: payer.publicKey,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .depositReserve({
          marketSideIndex: 0,
          depositAmount: new BN(1_000),
          minClaimAmount: new BN(900),
          maxBufferAmount: new BN(100),
        })
        .accounts({
          market: fixture.market,
          owner: payer.publicKey,
          assetMint: fixture.asset0Mint,
          claimMint: fixture.claim0Mint,
          reserveVault: fixture.reserve0Vault,
          ownerAssetAccount: ownerAsset0Account,
          ownerClaimAccount: ownerClaim0Account,
          stakePosition: stake0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority: fixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );
  });

  it("deposits reserve inventory and redeems fixed principal", async () => {
    trackInstruction("depositReserve", "deposits reserve inventory");
    trackInstruction("redeemClaim", "redeems fixed principal");

    const {
      asset0Mint,
      claim0Mint,
      market,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      eventAuthority,
    } = await fundTwoSidedMarket();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(1_000_000)
    );
    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(800_000)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(1_000_000)
    );

    await program.methods
      .redeemClaim({
        marketSideIndex: 0,
        claimAmount: new BN(80_000),
        minAssetAmountOut: new BN(80_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        claimMint: claim0Mint,
        reserveVault: reserve0Vault,
        ownerAssetAccount: ownerAsset0Account,
        ownerClaimAccount: ownerClaim0Account,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(1_080_000)
    );
    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(720_000)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(920_000)
    );
  });

  it("swaps against market reserve floor excess", async () => {
    trackInstruction("marketSwap", "swaps against rounded market reserve excess");

    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    await program.methods
      .marketSwap({
        assetInIsAsset0: true,
        exactAssetIn: new BN(3),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: asset0Mint,
        assetOutMint: asset1Mint,
        reserveInVault: reserve0Vault,
        reserveOutVault: reserve1Vault,
        feeInVault: fee0Vault,
        traderAssetInAccount: ownerAsset0Account,
        traderAssetOutAccount: ownerAsset1Account,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(91)
    );
    expect((await getAccount(connection as any, ownerAsset1Account)).amount).to.equal(
      BigInt(95)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(8)
    );
    expect((await getAccount(connection as any, reserve1Vault)).amount).to.equal(
      BigInt(5)
    );
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(1));
  });

  it("stakes and unstakes matched market claims and buffer shares", async () => {
    trackInstruction("stake", "stakes matched market claim and buffer shares");
    trackInstruction("unstake", "unstakes matched market claim and buffer shares");

    const {
      asset0Mint,
      claim0Mint,
      market,
      claim0StakeVault,
      ownerClaim0Account,
      stake0Position,
      eventAuthority,
    } = await fundTwoSidedMarket();

    await program.methods
      .stake({
        marketSideIndex: 0,
        claimAmount: new BN(400_000),
        bufferShares: new BN(100_000),
        minActiveStakeUnits: new BN(500_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        claimMint: claim0Mint,
        stakeVault: claim0StakeVault,
        ownerClaimAccount: ownerClaim0Account,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(400_000)
    );
    expect((await getAccount(connection as any, claim0StakeVault)).amount).to.equal(
      BigInt(400_000)
    );

    await program.methods
      .unstake({
        marketSideIndex: 0,
        claimAmount: new BN(160_000),
        bufferShares: new BN(40_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        claimMint: claim0Mint,
        stakeVault: claim0StakeVault,
        ownerClaimAccount: ownerClaim0Account,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(560_000)
    );
    expect((await getAccount(connection as any, claim0StakeVault)).amount).to.equal(
      BigInt(240_000)
    );
  });

  it("opens and closes hedged market claim wrappers", async () => {
    trackInstruction("openHedge", "wraps market claims into hedged claim tokens");
    trackInstruction("closeHedge", "unwraps hedged claim tokens into market claims");

    const {
      asset0Mint,
      claim0Mint,
      hedge0Mint,
      market,
      hedge0Vault,
      ownerClaim0Account,
      eventAuthority,
    } = await fundTwoSidedMarket();
    const ownerHedge0Account = await createAccount(
      connection as any,
      payer,
      hedge0Mint,
      payer.publicKey
    );
    const hedge0Position = deriveAddress(
      Buffer.from("hedge_position"),
      market.toBuffer(),
      payer.publicKey.toBuffer(),
      asset0Mint.toBuffer()
    );

    await program.methods
      .openHedge({
        marketSideIndex: 0,
        claimAmount: new BN(200_000),
        minHedgeAmount: new BN(200_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        claimMint: claim0Mint,
        hedgeMint: hedge0Mint,
        hedgeVault: hedge0Vault,
        ownerClaimAccount: ownerClaim0Account,
        ownerHedgeAccount: ownerHedge0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(600_000)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(200_000)
    );
    expect((await getAccount(connection as any, hedge0Vault)).amount).to.equal(
      BigInt(200_000)
    );

    await program.methods
      .closeHedge({
        marketSideIndex: 0,
        hedgeAmount: new BN(50_000),
        minClaimAmountOut: new BN(50_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        claimMint: claim0Mint,
        hedgeMint: hedge0Mint,
        hedgeVault: hedge0Vault,
        ownerClaimAccount: ownerClaim0Account,
        ownerHedgeAccount: ownerHedge0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(650_000)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(150_000)
    );
    expect((await getAccount(connection as any, hedge0Vault)).amount).to.equal(
      BigInt(150_000)
    );
  });

  it("deposits collateral and funds market insurance", async () => {
    trackInstruction("depositCollateral", "deposits borrower collateral");
    trackInstruction("depositInsurance", "funds market insurance reserves");

    const {
      asset0Mint,
      asset1Mint,
      market,
      collateral1Vault,
      insurance0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundTwoSidedMarket();

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(300_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition: deriveAddress(
          Buffer.from("margin"),
          market.toBuffer(),
          payer.publicKey.toBuffer()
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset1Account)).amount).to.equal(
      BigInt(700_000)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(300_000)
    );

    await program.methods
      .depositInsurance({
        marketSideIndex: 0,
        depositAmount: new BN(125_000),
      })
      .accounts({
        market,
        sponsor: payer.publicKey,
        assetMint: asset0Mint,
        insuranceVault: insurance0Vault,
        sponsorAssetAccount: ownerAsset0Account,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(875_000)
    );
    expect((await getAccount(connection as any, insurance0Vault)).amount).to.equal(
      BigInt(125_000)
    );
  });

});

after(() => {
  getCoverageReport();
});
