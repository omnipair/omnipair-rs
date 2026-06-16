import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import anchor from "@coral-xyz/anchor";
import {
  createAccount,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createMint,
  ExtensionType,
  getAccount,
  getMintLen,
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
  Transaction,
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
    kEmaDrawdownBps: 1_000,
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
  let svm: LiteSVM;

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

  async function createTransferFeeMint(
    decimals = 6,
    transferFeeBasisPoints = 1_000,
    maximumFee = 1_000_000,
    mintAuthority = payer.publicKey
  ) {
    const mint = Keypair.generate();
    const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
    const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferFeeConfigInstruction(
        mint.publicKey,
        payer.publicKey,
        payer.publicKey,
        transferFeeBasisPoints,
        BigInt(maximumFee),
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        mintAuthority,
        null,
        TOKEN_2022_PROGRAM_ID
      )
    );
    await connection.sendTransaction(tx, [payer, mint]);

    return mint.publicKey;
  }

  async function initializeTransferFeeMarketFixture() {
    const transferFeeMint = await createTransferFeeMint();
    const vanillaMint = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const [asset0Mint, asset1Mint] = orderedMints(transferFeeMint, vanillaMint);
    const transferFeeMarketSideIndex = asset0Mint.equals(transferFeeMint) ? 0 : 1;
    const paramsHash = Buffer.alloc(32, 8);
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
      market,
      reserve0Vault,
      reserve1Vault,
      collateral0Vault,
      collateral1Vault,
      fee0Vault,
      fee1Vault,
      eventAuthority,
      transferFeeMint,
      transferFeeMarketSideIndex,
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
    maxBufferAmount = 200_000,
    owner = payer
  ) {
    const stakePosition = deriveAddress(
      Buffer.from("stake"),
      fixture.market.toBuffer(),
      owner.publicKey.toBuffer(),
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
        owner: owner.publicKey,
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
      .signers([owner])
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

  async function fundRoundedBorrowMarket(rounds = 12) {
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
      300
    );
    await mintTo(
      connection as any,
      payer,
      fixture.asset1Mint,
      ownerAsset1Account,
      payer,
      300
    );

    for (let i = 0; i < rounds; i++) {
      const lender = Keypair.generate();
      await connection.requestAirdrop(lender.publicKey, LAMPORTS_PER_SOL);
      const lenderAsset0Account = await createAccount(
        connection as any,
        payer,
        fixture.asset0Mint,
        lender.publicKey
      );
      const lenderAsset1Account = await createAccount(
        connection as any,
        payer,
        fixture.asset1Mint,
        lender.publicKey
      );
      const lenderClaim0Account = await createAccount(
        connection as any,
        payer,
        fixture.claim0Mint,
        lender.publicKey
      );
      const lenderClaim1Account = await createAccount(
        connection as any,
        payer,
        fixture.claim1Mint,
        lender.publicKey
      );

      await mintTo(
        connection as any,
        payer,
        fixture.asset0Mint,
        lenderAsset0Account,
        payer,
        26
      );
      await mintTo(
        connection as any,
        payer,
        fixture.asset1Mint,
        lenderAsset1Account,
        payer,
        26
      );

      await depositReserveSide(
        fixture,
        0,
        fixture.asset0Mint,
        fixture.claim0Mint,
        fixture.reserve0Vault,
        lenderAsset0Account,
        lenderClaim0Account,
        26,
        20,
        6,
        lender
      );
      await depositReserveSide(
        fixture,
        1,
        fixture.asset1Mint,
        fixture.claim1Mint,
        fixture.reserve1Vault,
        lenderAsset1Account,
        lenderClaim1Account,
        26,
        20,
        6,
        lender
      );
    }

    return {
      ...fixture,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      ownerClaim1Account,
    };
  }

  it("rejects transfer-fee claim and hedge mints at market initialization", async () => {
    async function expectTransferFeeMintRejected(blockedMintKind: "claim" | "hedge", paramsSeed: number) {
      const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const [asset0Mint, asset1Mint] = orderedMints(mintA, mintB);
      const paramsHash = Buffer.alloc(32, paramsSeed);
      const [market] = PublicKey.findProgramAddressSync(
        [Buffer.from("market_v2"), asset0Mint.toBuffer(), asset1Mint.toBuffer(), paramsHash],
        OMNIPAIR_PROGRAM_ID
      );
      const [eventAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from("__event_authority")],
        OMNIPAIR_PROGRAM_ID
      );

      const blockedMint = await createTransferFeeMint(6, 1_000, 1_000_000, market);
      const claim0Mint = blockedMintKind === "claim"
        ? blockedMint
        : await createMint(connection as any, payer, market, null, 6);
      const claim1Mint = await createMint(connection as any, payer, market, null, 6);
      const hedge0Mint = blockedMintKind === "hedge"
        ? blockedMint
        : await createMint(connection as any, payer, market, null, 6);
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

      await expectRejects(() =>
        program.methods
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
          .rpc()
      );

      expect(await connection.getAccountInfo(market)).to.equal(null);
    }

    await expectTransferFeeMintRejected("claim", 20);
    await expectTransferFeeMintRejected("hedge", 21);
  });

  it("rejects default market operator and manager authorities", async () => {
    async function expectDefaultAuthorityRejected(operator: PublicKey, manager: PublicKey, paramsSeed: number) {
      const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const [asset0Mint, asset1Mint] = orderedMints(mintA, mintB);
      const paramsHash = Buffer.alloc(32, paramsSeed);
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

      await expectRejects(() =>
        program.methods
          .initializeMarket({
            operator,
            manager,
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
          .rpc()
      );

      expect(await connection.getAccountInfo(market)).to.equal(null);
    }

    await expectDefaultAuthorityRejected(SystemProgram.programId, payer.publicKey, 22);
    await expectDefaultAuthorityRejected(payer.publicKey, SystemProgram.programId, 23);
  });

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

  it("rejects disabled soft-borrow and unsafe health config updates", async () => {
    const { market, eventAuthority } = await initializeMarketFixture();

    const softBorrowConfig = marketConfig();
    softBorrowConfig.softBorrowEnabled = true;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config: softBorrowConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const unsafeHealthConfig = marketConfig();
    unsafeHealthConfig.recognizedCollateralCapBps = 10_500;
    unsafeHealthConfig.marketHealthMinBps = 11_000;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config: unsafeHealthConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const validConfig = marketConfig();
    validConfig.swapFeeBps = 42;
    await program.methods
      .updateMarketConfig({ config: validConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
  });

  it("rejects non-operator market administration", async () => {
    const { market, eventAuthority } = await initializeMarketFixture();
    const impostor = Keypair.generate();
    await connection.requestAirdrop(impostor.publicKey, LAMPORTS_PER_SOL);

    const config = marketConfig();
    config.swapFeeBps = 42;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config })
        .accounts({
          market,
          operator: impostor.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );

    await expectRejects(() =>
      program.methods
        .setMarketReduceOnly({ reduceOnly: true })
        .accounts({
          market,
          operator: impostor.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );
  });

  it("locks buffer-ratio updates while market stake is active", async () => {
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

    const lockedConfig = marketConfig();
    lockedConfig.bufferRatioBps = 1_500;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config: lockedConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(400_000)
    );
    expect((await getAccount(connection as any, claim0StakeVault)).amount).to.equal(
      BigInt(400_000)
    );
  });

  it("locks buffer-ratio updates while staker fee liability is outstanding", async () => {
    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      claim0StakeVault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketSideIndex: 0,
        claimAmount: new BN(4),
        bufferShares: new BN(1),
        minActiveStakeUnits: new BN(5),
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

    await program.methods
      .marketSwap({
        assetInIsAsset0: true,
        exactAssetIn: new BN(12),
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

    await program.methods
      .unstake({
        marketSideIndex: 0,
        claimAmount: new BN(4),
        bufferShares: new BN(1),
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

    const lockedConfig = marketConfig();
    lockedConfig.swapFeeBps = config.swapFeeBps;
    lockedConfig.bufferRatioBps = 1_500;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config: lockedConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(4)
    );
    expect((await getAccount(connection as any, claim0StakeVault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(10));
  });

  it("rejects buffer-ratio updates when the recomputed floor is uncovered", async () => {
    const {
      asset0Mint,
      claim0Mint,
      market,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      eventAuthority,
    } = await fundTwoSidedMarket();

    const uncoveredConfig = marketConfig();
    uncoveredConfig.bufferRatioBps = 2_500;
    await expectRejects(() =>
      program.methods
        .updateMarketConfig({ config: uncoveredConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await depositReserveSide(
      { market, eventAuthority },
      0,
      asset0Mint,
      claim0Mint,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      1_000,
      800,
      200
    );

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(800_800)
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

  it("accounts for Token-2022 transfer fees with market inventory credits", async () => {
    const fixture = await initializeTransferFeeMarketFixture();
    const tokenProgramForMint = (mint: PublicKey) =>
      mint.equals(fixture.transferFeeMint) ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      claim1Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      collateral0Vault,
      collateral1Vault,
      fee0Vault,
      fee1Vault,
      eventAuthority,
      transferFeeMarketSideIndex,
    } = fixture;
    const ownerAsset0Account = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      payer.publicKey,
      undefined,
      undefined,
      tokenProgramForMint(asset0Mint)
    );
    const ownerAsset1Account = await createAccount(
      connection as any,
      payer,
      asset1Mint,
      payer.publicKey,
      undefined,
      undefined,
      tokenProgramForMint(asset1Mint)
    );
    const ownerClaim0Account = await createAccount(
      connection as any,
      payer,
      claim0Mint,
      payer.publicKey
    );
    const ownerClaim1Account = await createAccount(
      connection as any,
      payer,
      claim1Mint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      asset0Mint,
      ownerAsset0Account,
      payer,
      2_000,
      [],
      undefined,
      tokenProgramForMint(asset0Mint)
    );
    await mintTo(
      connection as any,
      payer,
      asset1Mint,
      ownerAsset1Account,
      payer,
      2_000,
      [],
      undefined,
      tokenProgramForMint(asset1Mint)
    );

    const transferFeeSide = transferFeeMarketSideIndex === 0
      ? {
          assetMint: asset0Mint,
          claimMint: claim0Mint,
          reserveVault: reserve0Vault,
          ownerAssetAccount: ownerAsset0Account,
          ownerClaimAccount: ownerClaim0Account,
          collateralVault: collateral1Vault,
          collateralAssetMint: asset1Mint,
          collateralOwnerAccount: ownerAsset1Account,
          borrowAssetIsAsset0: true,
        }
      : {
          assetMint: asset1Mint,
          claimMint: claim1Mint,
          reserveVault: reserve1Vault,
          ownerAssetAccount: ownerAsset1Account,
          ownerClaimAccount: ownerClaim1Account,
          collateralVault: collateral0Vault,
          collateralAssetMint: asset0Mint,
          collateralOwnerAccount: ownerAsset0Account,
          borrowAssetIsAsset0: false,
        };
    const vanillaSide = transferFeeMarketSideIndex === 0
      ? {
          marketSideIndex: 1,
          assetMint: asset1Mint,
          claimMint: claim1Mint,
          reserveVault: reserve1Vault,
          feeVault: fee1Vault,
          ownerAssetAccount: ownerAsset1Account,
          ownerClaimAccount: ownerClaim1Account,
        }
      : {
          marketSideIndex: 0,
          assetMint: asset0Mint,
          claimMint: claim0Mint,
          reserveVault: reserve0Vault,
          feeVault: fee0Vault,
          ownerAssetAccount: ownerAsset0Account,
          ownerClaimAccount: ownerClaim0Account,
        };

    await depositReserveSide(
      fixture,
      transferFeeMarketSideIndex,
      transferFeeSide.assetMint,
      transferFeeSide.claimMint,
      transferFeeSide.reserveVault,
      transferFeeSide.ownerAssetAccount,
      transferFeeSide.ownerClaimAccount,
      1_000,
      720,
      180
    );
    for (let i = 0; i < 10; i++) {
      const lender = Keypair.generate();
      await connection.requestAirdrop(lender.publicKey, LAMPORTS_PER_SOL);
      const lenderAssetAccount = await createAccount(
        connection as any,
        payer,
        transferFeeSide.assetMint,
        lender.publicKey,
        undefined,
        undefined,
        TOKEN_2022_PROGRAM_ID
      );
      const lenderClaimAccount = await createAccount(
        connection as any,
        payer,
        transferFeeSide.claimMint,
        lender.publicKey
      );
      await mintTo(
        connection as any,
        payer,
        transferFeeSide.assetMint,
        lenderAssetAccount,
        payer,
        29,
        [],
        undefined,
        TOKEN_2022_PROGRAM_ID
      );
      await depositReserveSide(
        fixture,
        transferFeeMarketSideIndex,
        transferFeeSide.assetMint,
        transferFeeSide.claimMint,
        transferFeeSide.reserveVault,
        lenderAssetAccount,
        lenderClaimAccount,
        29,
        20,
        6,
        lender
      );
    }
    await depositReserveSide(
      fixture,
      vanillaSide.marketSideIndex,
      vanillaSide.assetMint,
      vanillaSide.claimMint,
      vanillaSide.reserveVault,
      vanillaSide.ownerAssetAccount,
      vanillaSide.ownerClaimAccount,
      1_000,
      800,
      200
    );

    expect(
      (await getAccount(
        connection as any,
        transferFeeSide.reserveVault,
        undefined,
        TOKEN_2022_PROGRAM_ID
      )).amount
    ).to.equal(BigInt(1_160));
    expect(
      (await getAccount(connection as any, transferFeeSide.ownerClaimAccount)).amount
    ).to.equal(BigInt(720));

    await expectRejects(() =>
      program.methods
        .redeemClaim({
          marketSideIndex: transferFeeMarketSideIndex,
          claimAmount: new BN(5),
          minAssetAmountOut: new BN(5),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: transferFeeSide.assetMint,
          claimMint: transferFeeSide.claimMint,
          reserveVault: transferFeeSide.reserveVault,
          ownerAssetAccount: transferFeeSide.ownerAssetAccount,
          ownerClaimAccount: transferFeeSide.ownerClaimAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const ownerRedeemBalanceBefore = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    await program.methods
      .redeemClaim({
        marketSideIndex: transferFeeMarketSideIndex,
        claimAmount: new BN(5),
        minAssetAmountOut: new BN(4),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: transferFeeSide.assetMint,
        claimMint: transferFeeSide.claimMint,
        reserveVault: transferFeeSide.reserveVault,
        ownerAssetAccount: transferFeeSide.ownerAssetAccount,
        ownerClaimAccount: transferFeeSide.ownerClaimAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const ownerRedeemBalanceAfter = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    expect(ownerRedeemBalanceAfter - ownerRedeemBalanceBefore).to.equal(BigInt(4));

    await expectRejects(() =>
      program.methods
        .marketSwap({
          assetInIsAsset0: !transferFeeSide.borrowAssetIsAsset0,
          exactAssetIn: new BN(5),
          minAssetOut: new BN(4),
        })
        .accounts({
          market,
          trader: payer.publicKey,
          assetInMint: vanillaSide.assetMint,
          assetOutMint: transferFeeSide.assetMint,
          reserveInVault: vanillaSide.reserveVault,
          reserveOutVault: transferFeeSide.reserveVault,
          feeInVault: vanillaSide.feeVault,
          traderAssetInAccount: vanillaSide.ownerAssetAccount,
          traderAssetOutAccount: transferFeeSide.ownerAssetAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const ownerSwapBalanceBefore = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    await program.methods
      .marketSwap({
        assetInIsAsset0: !transferFeeSide.borrowAssetIsAsset0,
        exactAssetIn: new BN(5),
        minAssetOut: new BN(3),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: vanillaSide.assetMint,
        assetOutMint: transferFeeSide.assetMint,
        reserveInVault: vanillaSide.reserveVault,
        reserveOutVault: transferFeeSide.reserveVault,
        feeInVault: vanillaSide.feeVault,
        traderAssetInAccount: vanillaSide.ownerAssetAccount,
        traderAssetOutAccount: transferFeeSide.ownerAssetAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const ownerSwapBalanceAfter = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    expect(ownerSwapBalanceAfter - ownerSwapBalanceBefore).to.equal(BigInt(3));

    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );
    await program.methods
      .depositCollateral({
        marketSideIndex: transferFeeMarketSideIndex === 0 ? 1 : 0,
        depositAmount: new BN(300),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: transferFeeSide.collateralAssetMint,
        collateralVault: transferFeeSide.collateralVault,
        ownerAssetAccount: transferFeeSide.collateralOwnerAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: transferFeeSide.borrowAssetIsAsset0,
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(5),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: transferFeeSide.assetMint,
          collateralAssetMint: transferFeeSide.collateralAssetMint,
          reserveVault: transferFeeSide.reserveVault,
          ownerDebtAccount: transferFeeSide.ownerAssetAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    const ownerDebtBalanceBefore = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: transferFeeSide.borrowAssetIsAsset0,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(4),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: transferFeeSide.assetMint,
        collateralAssetMint: transferFeeSide.collateralAssetMint,
        reserveVault: transferFeeSide.reserveVault,
        ownerDebtAccount: transferFeeSide.ownerAssetAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();
    const ownerDebtBalanceAfter = (await getAccount(
      connection as any,
      transferFeeSide.ownerAssetAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    )).amount;
    expect(ownerDebtBalanceAfter - ownerDebtBalanceBefore).to.equal(BigInt(4));
  });

  it("enforces daily borrow, redeem, and collateral withdraw limits from liquidity EMA", async () => {
    const borrowFixture = await fundRoundedBorrowMarket();
    const borrowLimitConfig = marketConfig();
    borrowLimitConfig.maxDailyBorrowBps = 300;
    await program.methods
      .updateMarketConfig({ config: borrowLimitConfig })
      .accounts({
        market: borrowFixture.market,
        operator: payer.publicKey,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      borrowFixture.market.toBuffer(),
      payer.publicKey.toBuffer()
    );
    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(200),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        assetMint: borrowFixture.asset1Mint,
        collateralVault: borrowFixture.collateral1Vault,
        ownerAssetAccount: borrowFixture.ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(9),
        minDebtAmountOut: new BN(9),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        debtAssetMint: borrowFixture.asset0Mint,
        collateralAssetMint: borrowFixture.asset1Mint,
        reserveVault: borrowFixture.reserve0Vault,
        ownerDebtAccount: borrowFixture.ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: true,
          borrowAmount: new BN(1),
          minDebtAmountOut: new BN(1),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market: borrowFixture.market,
          owner: payer.publicKey,
          debtAssetMint: borrowFixture.asset0Mint,
          collateralAssetMint: borrowFixture.asset1Mint,
          reserveVault: borrowFixture.reserve0Vault,
          ownerDebtAccount: borrowFixture.ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: borrowFixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    const redeemFixture = await fundTwoSidedMarket();
    const redeemLimitConfig = marketConfig();
    redeemLimitConfig.maxDailyWithdrawBps = 1;
    await program.methods
      .updateMarketConfig({ config: redeemLimitConfig })
      .accounts({
        market: redeemFixture.market,
        operator: payer.publicKey,
        eventAuthority: redeemFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .redeemClaim({
        marketSideIndex: 0,
        claimAmount: new BN(100),
        minAssetAmountOut: new BN(100),
      })
      .accounts({
        market: redeemFixture.market,
        owner: payer.publicKey,
        assetMint: redeemFixture.asset0Mint,
        claimMint: redeemFixture.claim0Mint,
        reserveVault: redeemFixture.reserve0Vault,
        ownerAssetAccount: redeemFixture.ownerAsset0Account,
        ownerClaimAccount: redeemFixture.ownerClaim0Account,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: redeemFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .redeemClaim({
          marketSideIndex: 0,
          claimAmount: new BN(1),
          minAssetAmountOut: new BN(1),
        })
        .accounts({
          market: redeemFixture.market,
          owner: payer.publicKey,
          assetMint: redeemFixture.asset0Mint,
          claimMint: redeemFixture.claim0Mint,
          reserveVault: redeemFixture.reserve0Vault,
          ownerAssetAccount: redeemFixture.ownerAsset0Account,
          ownerClaimAccount: redeemFixture.ownerClaim0Account,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: redeemFixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const collateralWithdrawFixture = await fundRoundedBorrowMarket();
    const collateralWithdrawLimitConfig = marketConfig();
    collateralWithdrawLimitConfig.maxDailyWithdrawBps = 300;
    await program.methods
      .updateMarketConfig({ config: collateralWithdrawLimitConfig })
      .accounts({
        market: collateralWithdrawFixture.market,
        operator: payer.publicKey,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const collateralWithdrawMarginPosition = deriveAddress(
      Buffer.from("margin"),
      collateralWithdrawFixture.market.toBuffer(),
      payer.publicKey.toBuffer()
    );
    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(20),
      })
      .accounts({
        market: collateralWithdrawFixture.market,
        owner: payer.publicKey,
        assetMint: collateralWithdrawFixture.asset1Mint,
        collateralVault: collateralWithdrawFixture.collateral1Vault,
        ownerAssetAccount: collateralWithdrawFixture.ownerAsset1Account,
        marginPosition: collateralWithdrawMarginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .withdrawCollateral({
        marketSideIndex: 1,
        withdrawAmount: new BN(9),
        minAssetAmountOut: new BN(9),
      })
      .accounts({
        market: collateralWithdrawFixture.market,
        owner: payer.publicKey,
        assetMint: collateralWithdrawFixture.asset1Mint,
        collateralVault: collateralWithdrawFixture.collateral1Vault,
        ownerAssetAccount: collateralWithdrawFixture.ownerAsset1Account,
        marginPosition: collateralWithdrawMarginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await expectRejects(() =>
      program.methods
        .withdrawCollateral({
          marketSideIndex: 1,
          withdrawAmount: new BN(1),
          minAssetAmountOut: new BN(1),
        })
        .accounts({
          market: collateralWithdrawFixture.market,
          owner: payer.publicKey,
          assetMint: collateralWithdrawFixture.asset1Mint,
          collateralVault: collateralWithdrawFixture.collateral1Vault,
          ownerAssetAccount: collateralWithdrawFixture.ownerAsset1Account,
          marginPosition: collateralWithdrawMarginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: collateralWithdrawFixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
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

  it("blocks market swaps and borrows in reduce-only mode", async () => {
    const swapFixture = await fundTinyRoundingMarket();

    await program.methods
      .setMarketReduceOnly({ reduceOnly: true })
      .accounts({
        market: swapFixture.market,
        operator: payer.publicKey,
        eventAuthority: swapFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketSwap({
          assetInIsAsset0: true,
          exactAssetIn: new BN(3),
          minAssetOut: new BN(1),
        })
        .accounts({
          market: swapFixture.market,
          trader: payer.publicKey,
          assetInMint: swapFixture.asset0Mint,
          assetOutMint: swapFixture.asset1Mint,
          reserveInVault: swapFixture.reserve0Vault,
          reserveOutVault: swapFixture.reserve1Vault,
          feeInVault: swapFixture.fee0Vault,
          traderAssetInAccount: swapFixture.ownerAsset0Account,
          traderAssetOutAccount: swapFixture.ownerAsset1Account,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: swapFixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const borrowFixture = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      borrowFixture.market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        assetMint: borrowFixture.asset1Mint,
        collateralVault: borrowFixture.collateral1Vault,
        ownerAssetAccount: borrowFixture.ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .setMarketReduceOnly({ reduceOnly: true })
      .accounts({
        market: borrowFixture.market,
        operator: payer.publicKey,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: true,
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(5),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market: borrowFixture.market,
          owner: payer.publicKey,
          debtAssetMint: borrowFixture.asset0Mint,
          collateralAssetMint: borrowFixture.asset1Mint,
          reserveVault: borrowFixture.reserve0Vault,
          ownerDebtAccount: borrowFixture.ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: borrowFixture.eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );
  });

  it("claims staker and operator market fees", async () => {
    trackInstruction("claimFees", "claims non-compounding staker fees");
    trackInstruction("claimMarketFees", "claims operator market fee liabilities");

    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      claim0StakeVault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketSideIndex: 0,
        claimAmount: new BN(4),
        bufferShares: new BN(1),
        minActiveStakeUnits: new BN(5),
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

    await program.methods
      .marketSwap({
        assetInIsAsset0: true,
        exactAssetIn: new BN(12),
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

    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(10));

    await program.methods
      .claimFees({
        marketSideIndex: 0,
        minFeeAmount: new BN(9),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        feeVault: fee0Vault,
        ownerFeeAccount: ownerAsset0Account,
        stakePosition: stake0Position,
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
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(1));

    const impostor = Keypair.generate();
    await connection.requestAirdrop(impostor.publicKey, LAMPORTS_PER_SOL);
    const impostorAsset0Account = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      impostor.publicKey
    );
    await expectRejects(() =>
      program.methods
        .claimMarketFees({
          marketSideIndex: 0,
          claimKind: { operator: {} },
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          feeAuthority: impostor.publicKey,
          assetMint: asset0Mint,
          feeVault: fee0Vault,
          recipientFeeAccount: impostorAsset0Account,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );

    expect((await getAccount(connection as any, impostorAsset0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(1));

    await program.methods
      .claimMarketFees({
        marketSideIndex: 0,
        claimKind: { operator: {} },
        minFeeAmount: new BN(1),
      })
      .accounts({
        market,
        feeAuthority: payer.publicKey,
        assetMint: asset0Mint,
        feeVault: fee0Vault,
        recipientFeeAccount: ownerAsset0Account,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(92)
    );
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(0));
  });

  it("carries no-stake LP fees into the next active market stake", async () => {
    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      claim0StakeVault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketSwap({
        assetInIsAsset0: true,
        exactAssetIn: new BN(12),
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

    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(10));

    await program.methods
      .stake({
        marketSideIndex: 0,
        claimAmount: new BN(4),
        bufferShares: new BN(1),
        minActiveStakeUnits: new BN(5),
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

    await program.methods
      .claimFees({
        marketSideIndex: 0,
        minFeeAmount: new BN(9),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        feeVault: fee0Vault,
        ownerFeeAccount: ownerAsset0Account,
        stakePosition: stake0Position,
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
    expect((await getAccount(connection as any, fee0Vault)).amount).to.equal(BigInt(1));
  });

  it("blocks fee claims when spot diverges from cached EMA", async () => {
    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      claim0StakeVault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketSideIndex: 0,
        claimAmount: new BN(4),
        bufferShares: new BN(1),
        minActiveStakeUnits: new BN(5),
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

    await program.methods
      .marketSwap({
        assetInIsAsset0: true,
        exactAssetIn: new BN(12),
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

    await depositReserveSide(
      { market, eventAuthority },
      0,
      asset0Mint,
      claim0Mint,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      50,
      40,
      10
    );

    await expectRejects(() =>
      program.methods
        .claimFees({
          marketSideIndex: 0,
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: asset0Mint,
          feeVault: fee0Vault,
          ownerFeeAccount: ownerAsset0Account,
          stakePosition: stake0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await expectRejects(() =>
      program.methods
        .claimMarketFees({
          marketSideIndex: 0,
          claimKind: { operator: {} },
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          feeAuthority: payer.publicKey,
          assetMint: asset0Mint,
          feeVault: fee0Vault,
          recipientFeeAccount: ownerAsset0Account,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );
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
    trackInstruction("claimHedgeFees", "claims routed hedged market fees");
    trackInstruction("closeHedge", "unwraps hedged claim tokens into market claims");

    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      hedge0Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      fee0Vault,
      hedge0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      eventAuthority,
    } = await fundTinyRoundingMarket();
    const hedgeFeeConfig = marketConfig();
    hedgeFeeConfig.spotEmaDivergenceBps = 10_000;
    hedgeFeeConfig.kEmaDrawdownBps = 10_000;
    await program.methods
      .updateMarketConfig({ config: hedgeFeeConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

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
        claimAmount: new BN(1),
        minHedgeAmount: new BN(1),
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
      BigInt(3)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(1)
    );
    expect((await getAccount(connection as any, hedge0Vault)).amount).to.equal(
      BigInt(1)
    );

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

    const ownerAsset0BeforeHedgeFeeClaim = (await getAccount(
      connection as any,
      ownerAsset0Account
    )).amount;
    await program.methods
      .claimHedgeFees({
        marketSideIndex: 0,
        minFeeAmount: new BN(1),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        feeVault: fee0Vault,
        ownerFeeAccount: ownerAsset0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    expect(
      (await getAccount(connection as any, ownerAsset0Account)).amount >
        ownerAsset0BeforeHedgeFeeClaim
    ).to.equal(true);

    await program.methods
      .setMarketReduceOnly({ reduceOnly: true })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .openHedge({
          marketSideIndex: 0,
          claimAmount: new BN(1),
          minHedgeAmount: new BN(1),
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
        .rpc()
    );

    await program.methods
      .closeHedge({
        marketSideIndex: 0,
        hedgeAmount: new BN(1),
        minClaimAmountOut: new BN(1),
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
      BigInt(4)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, hedge0Vault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("blocks hedged market claim wrappers when disabled by config", async () => {
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

    const config = marketConfig();
    config.hedgedLpEnabled = false;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .openHedge({
          marketSideIndex: 0,
          claimAmount: new BN(50_000),
          minHedgeAmount: new BN(50_000),
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
        .rpc()
    );

    expect((await getAccount(connection as any, ownerClaim0Account)).amount).to.equal(
      BigInt(800_000)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, hedge0Vault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("blocks hedge closes when spot diverges from cached EMA", async () => {
    const {
      asset0Mint,
      claim0Mint,
      hedge0Mint,
      market,
      reserve0Vault,
      hedge0Vault,
      ownerAsset0Account,
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

    await depositReserveSide(
      { market, eventAuthority },
      0,
      asset0Mint,
      claim0Mint,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      500_000,
      400_000,
      100_000
    );

    await expectRejects(() =>
      program.methods
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
        .rpc()
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

  it("borrows and repays fixed market debt against recognized collateral", async () => {
    trackInstruction("marketBorrow", "borrows fixed market debt");
    trackInstruction("marketRepay", "repays fixed market debt");
    trackInstruction("withdrawCollateral", "withdraws idle borrower collateral");

    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: true,
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(6),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          ownerDebtAccount: ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(305)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(307)
    );

    await expectRejects(() =>
      program.methods
        .withdrawCollateral({
          marketSideIndex: 1,
          withdrawAmount: new BN(60),
          minAssetAmountOut: new BN(60),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: asset1Mint,
          collateralVault: collateral1Vault,
          ownerAssetAccount: ownerAsset1Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await program.methods
      .marketRepay({
        repayAssetIsAsset0: true,
        repayAmount: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, ownerAsset1Account)).amount).to.equal(
      BigInt(240)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(60)
    );

    await program.methods
      .withdrawCollateral({
        marketSideIndex: 1,
        withdrawAmount: new BN(60),
        minAssetAmountOut: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    expect((await getAccount(connection as any, ownerAsset1Account)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("rejects fixed market borrows below required position health", async () => {
    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: true,
          borrowAmount: new BN(12),
          minDebtAmountOut: new BN(12),
          minHealthBps: new BN(20_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          ownerDebtAccount: ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(312)
    );
  });

  it("rejects fixed market borrows against same-side idle collateral", async () => {
    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral0Vault,
      ownerAsset0Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 0,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset0Mint,
        collateralVault: collateral0Vault,
        ownerAssetAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .marketBorrow({
          borrowAssetIsAsset0: true,
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(5),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          ownerDebtAccount: ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerAsset0Account)).amount).to.equal(
      BigInt(240)
    );
    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, collateral0Vault)).amount).to.equal(
      BigInt(60)
    );
  });

  it("blocks market repays when spot diverges from cached EMA", async () => {
    const {
      asset0Mint,
      asset1Mint,
      claim0Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim0Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await depositReserveSide(
      { market, eventAuthority },
      0,
      asset0Mint,
      claim0Mint,
      reserve0Vault,
      ownerAsset0Account,
      ownerClaim0Account,
      260,
      208,
      52
    );

    await expectRejects(() =>
      program.methods
        .marketRepay({
          repayAssetIsAsset0: true,
          repayAmount: new BN(5),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: asset0Mint,
          reserveVault: reserve0Vault,
          ownerDebtAccount: ownerAsset0Account,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );
  });

  async function fundExhaustibleLiquidationMarket(insuranceAmount = 0) {
    const fixture = await fundRoundedBorrowMarket();
    const {
      asset0Mint,
      asset1Mint,
      claim1Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      collateral1Vault,
      insurance0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim1Account,
      eventAuthority,
    } = fixture;
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(6),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const config = marketConfig();
    config.recognizedCollateralCapBps = 20_000;
    config.marketHealthMinBps = 20_000;
    config.spotEmaDivergenceBps = 10_000;
    config.kEmaDrawdownBps = 10_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await mintTo(
      connection as any,
      payer,
      asset1Mint,
      ownerAsset1Account,
      payer,
      300
    );
    await depositReserveSide(
      { market, eventAuthority },
      1,
      asset1Mint,
      claim1Mint,
      reserve1Vault,
      ownerAsset1Account,
      ownerClaim1Account,
      300,
      240,
      60
    );

    if (insuranceAmount > 0) {
      await program.methods
        .depositInsurance({
          marketSideIndex: 0,
          depositAmount: new BN(insuranceAmount),
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
    }

    const currentSlot = svm.getClock().slot;
    svm.warpToSlot(currentSlot + 1_500n);

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      asset1Mint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      asset0Mint,
      liquidatorDebtAccount,
      payer,
      4
    );

    return {
      ...fixture,
      marginPosition,
      liquidator,
      liquidatorDebtAccount,
      liquidatorCollateralAccount,
    };
  }

  it("rejects liquidations while fixed market debt is healthy", async () => {
    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      insurance0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      asset1Mint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      asset0Mint,
      liquidatorDebtAccount,
      payer,
      10
    );

    await expectRejects(() =>
      program.methods
        .marketLiquidate({
          debtAssetIsAsset0: true,
          repayAmount: new BN(5),
          minCollateralOut: new BN(1),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          collateralVault: collateral1Vault,
          insuranceVault: insurance0Vault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(307)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(60)
    );
    expect((await getAccount(connection as any, liquidatorDebtAccount)).amount).to.equal(
      BigInt(10)
    );
    expect((await getAccount(connection as any, liquidatorCollateralAccount)).amount).to.equal(
      BigInt(0)
    );
  });

  it("liquidates unhealthy fixed market debt", async () => {
    trackInstruction("marketLiquidate", "liquidates fixed market debt");

    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      insurance0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const config = marketConfig();
    config.recognizedCollateralCapBps = 20_000;
    config.marketHealthMinBps = 20_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      asset1Mint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      asset0Mint,
      liquidatorDebtAccount,
      payer,
      10
    );

    await program.methods
      .marketLiquidate({
        debtAssetIsAsset0: true,
        repayAmount: new BN(5),
        minCollateralOut: new BN(1),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        collateralVault: collateral1Vault,
        insuranceVault: insurance0Vault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, liquidatorDebtAccount)).amount).to.equal(
      BigInt(5)
    );
    const liquidatorCollateralBalance = (await getAccount(
      connection as any,
      liquidatorCollateralAccount
    )).amount;
    const collateralVaultBalance = (await getAccount(
      connection as any,
      collateral1Vault
    )).amount;
    expect(liquidatorCollateralBalance > BigInt(0)).to.equal(true);
    expect(collateralVaultBalance < BigInt(60)).to.equal(true);
  });

  it("uses insurance when liquidation exhausts collateral", async () => {
    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      insurance0Vault,
      liquidator,
      liquidatorDebtAccount,
      liquidatorCollateralAccount,
      marginPosition,
      eventAuthority,
    } = await fundExhaustibleLiquidationMarket(1);

    await expectRejects(() =>
      program.methods
        .marketLiquidate({
          debtAssetIsAsset0: true,
          repayAmount: new BN(4),
          minCollateralOut: new BN(6),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          collateralVault: collateral1Vault,
          insuranceVault: insurance0Vault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    await program.methods
      .marketLiquidate({
        debtAssetIsAsset0: true,
        repayAmount: new BN(4),
        minCollateralOut: new BN(6),
        maxInsuranceDraw: new BN(1),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        collateralVault: collateral1Vault,
        insuranceVault: insurance0Vault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, insurance0Vault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, liquidatorDebtAccount)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, liquidatorCollateralAccount)).amount).to.equal(
      BigInt(6)
    );
  });

  it("socializes bad debt when exhausted collateral has no insurance", async () => {
    const {
      asset0Mint,
      asset1Mint,
      market,
      reserve0Vault,
      collateral1Vault,
      insurance0Vault,
      liquidator,
      liquidatorDebtAccount,
      liquidatorCollateralAccount,
      marginPosition,
      eventAuthority,
    } = await fundExhaustibleLiquidationMarket(0);

    await expectRejects(() =>
      program.methods
        .marketLiquidate({
          debtAssetIsAsset0: true,
          repayAmount: new BN(4),
          minCollateralOut: new BN(6),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          collateralVault: collateral1Vault,
          insuranceVault: insurance0Vault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    await program.methods
      .marketLiquidate({
        debtAssetIsAsset0: true,
        repayAmount: new BN(4),
        minCollateralOut: new BN(6),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(1),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        collateralVault: collateral1Vault,
        insuranceVault: insurance0Vault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    expect((await getAccount(connection as any, reserve0Vault)).amount).to.equal(
      BigInt(311)
    );
    expect((await getAccount(connection as any, insurance0Vault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, collateral1Vault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, liquidatorDebtAccount)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, liquidatorCollateralAccount)).amount).to.equal(
      BigInt(6)
    );
  });

  it("blocks market liquidations when spot diverges from cached EMA", async () => {
    const {
      asset0Mint,
      asset1Mint,
      claim1Mint,
      market,
      reserve0Vault,
      reserve1Vault,
      collateral1Vault,
      insurance0Vault,
      ownerAsset0Account,
      ownerAsset1Account,
      ownerClaim1Account,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketSideIndex: 1,
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: asset1Mint,
        collateralVault: collateral1Vault,
        ownerAssetAccount: ownerAsset1Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .marketBorrow({
        borrowAssetIsAsset0: true,
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: asset0Mint,
        collateralAssetMint: asset1Mint,
        reserveVault: reserve0Vault,
        ownerDebtAccount: ownerAsset0Account,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const config = marketConfig();
    config.recognizedCollateralCapBps = 20_000;
    config.marketHealthMinBps = 20_000;
    await program.methods
      .updateMarketConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await depositReserveSide(
      { market, eventAuthority },
      1,
      asset1Mint,
      claim1Mint,
      reserve1Vault,
      ownerAsset1Account,
      ownerClaim1Account,
      200,
      160,
      40
    );

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      asset0Mint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      asset1Mint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      asset0Mint,
      liquidatorDebtAccount,
      payer,
      10
    );

    await expectRejects(() =>
      program.methods
        .marketLiquidate({
          debtAssetIsAsset0: true,
          repayAmount: new BN(5),
          minCollateralOut: new BN(1),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: asset0Mint,
          collateralAssetMint: asset1Mint,
          reserveVault: reserve0Vault,
          collateralVault: collateral1Vault,
          insuranceVault: insurance0Vault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );
  });

});

after(() => {
  getCoverageReport();
});
