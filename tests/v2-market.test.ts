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
import {
  deriveHedgePositionAddress,
  deriveHedgeVaultAddress,
  deriveInsuranceReserveAddress,
  deriveMarginPositionAddress,
  deriveMarketAddress,
  deriveMarketCollateralVaultAddress,
  deriveMarketFeeVaultAddress,
  deriveMarketReserveVaultAddress,
  deriveMarketStakeVaultAddress,
  deriveStakePositionAddress,
} from "../packages/program-interface/src/constants.js";
import { LiteSVMConnection } from "./utils/litesvm-connection.js";
import { trackV2Instruction as trackInstruction, getCoverageReport } from "./utils/instruction-coverage.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OMNIPAIR_V2_PROGRAM_ID = new PublicKey("358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv");
const { AnchorProvider, BN, Program, Wallet } = anchor;
const NAD = new BN(1_000_000_000);
const BASE_MARKET_ASSET = { base: {} };
const QUOTE_MARKET_ASSET = { quote: {} };
const ANCHOR_EVENT_IX_TAG = Buffer.from([0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]);

const omnipairV2IdlPath = path.join(__dirname, "../target/idl/omnipair_v2.json");
const omnipairV2IdlData = JSON.parse(fs.readFileSync(omnipairV2IdlPath, "utf-8")) as any;
const omnipairV2Idl = {
  ...omnipairV2IdlData,
  accounts: [],
} as any;
const accountCoder = new anchor.BorshAccountsCoder(omnipairV2IdlData as any);

function orderedMints(mintA: PublicKey, mintB: PublicKey): [PublicKey, PublicKey] {
  return Buffer.compare(mintA.toBuffer(), mintB.toBuffer()) < 0
    ? [mintA, mintB]
    : [mintB, mintA];
}

function deriveAddress(...seeds: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, OMNIPAIR_V2_PROGRAM_ID)[0];
}

function marketAssetFromIndex(sideIndex: number) {
  return sideIndex === 0 ? BASE_MARKET_ASSET : QUOTE_MARKET_ASSET;
}

function oppositeMarketAssetFromIndex(sideIndex: number) {
  return marketAssetFromIndex(sideIndex === 0 ? 1 : 0);
}

function normalizeMarketAsset(marketAsset: number | { base?: {}; quote?: {} }) {
  return typeof marketAsset === "number" ? marketAssetFromIndex(marketAsset) : marketAsset;
}

function decodeCpiEvents(svm: LiteSVM, signature: string) {
  const transaction = svm.getTransaction(Buffer.from(signature, "base64"));
  expect(transaction).to.not.equal(null);
  const eventCoder = new anchor.BorshEventCoder(omnipairV2IdlData as any);
  const events: any[] = [];

  for (const instructionGroup of (transaction as any).innerInstructions()) {
    for (const innerInstruction of instructionGroup) {
      const data = Buffer.from(innerInstruction.instruction().data());
      if (!data.subarray(0, ANCHOR_EVENT_IX_TAG.length).equals(ANCHOR_EVENT_IX_TAG)) {
        continue;
      }
      const event = eventCoder.decode(data.subarray(ANCHOR_EVENT_IX_TAG.length).toString("base64"));
      if (event) {
        events.push(event);
      }
    }
  }

  return events;
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
    protocolFeeBps: 0,
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

function syntheticLiquidityRiskConfig() {
  const config = marketConfig();
  config.spotEmaDivergenceBps = 10_000;
  config.kEmaDrawdownBps = 10_000;
  return config;
}

describe("Omnipair Market LiteSVM", () => {
  let connection: LiteSVMConnection;
  let provider: any;
  let program: any;
  let payer: Keypair;
  let svm: LiteSVM;

  function advanceRiskEmaWindow() {
    const currentSlot = svm.getClock().slot;
    svm.warpToSlot(currentSlot + 1_500n);
  }

  before(async () => {
    svm = new LiteSVM();
    const programPath = path.join(__dirname, "../target/deploy/omnipair_v2.so");
    if (!fs.existsSync(programPath)) {
      throw new Error(`Program file not found at ${programPath}`);
    }

    svm.addProgramFromFile(OMNIPAIR_V2_PROGRAM_ID, programPath);
    connection = new LiteSVMConnection(svm);

    payer = Keypair.generate();
    await connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL);

    provider = new AnchorProvider(connection as any, new Wallet(payer) as any, {});
    program = new Program(omnipairV2Idl as any, provider as any);
  });

  async function initializeMarketFixture(
    mintOrder: "canonical" | "reversed" = "canonical",
    config = marketConfig()
  ) {
    const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const [lowerMint, higherMint] = orderedMints(mintA, mintB);
    const [baseMint, quoteMint] = mintOrder === "reversed"
      ? [higherMint, lowerMint]
      : [lowerMint, higherMint];
    const paramsHash = Buffer.alloc(32, 7);
    const [market] = PublicKey.findProgramAddressSync(
      [Buffer.from("market_v2"), baseMint.toBuffer(), quoteMint.toBuffer(), paramsHash],
      OMNIPAIR_V2_PROGRAM_ID
    );
    const [eventAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      OMNIPAIR_V2_PROGRAM_ID
    );
    const baseClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
    const quoteClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
    const baseHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
    const quoteHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
    const baseHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), baseClaimTokenMint.toBuffer());
    const quoteHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), quoteClaimTokenMint.toBuffer());
    const baseReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), baseMint.toBuffer());
    const quoteReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), quoteMint.toBuffer());
    const baseCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), baseMint.toBuffer());
    const quoteCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), quoteMint.toBuffer());
    const baseInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), baseMint.toBuffer());
    const quoteInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), quoteMint.toBuffer());
    const baseFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), baseMint.toBuffer());
    const quoteFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), quoteMint.toBuffer());
    const baseStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), baseClaimTokenMint.toBuffer());
    const quoteStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), quoteClaimTokenMint.toBuffer());

    const signature = await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config,
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        baseMint: baseMint,
        quoteMint: quoteMint,
        market,
        baseClaimTokenMint: baseClaimTokenMint,
        quoteClaimTokenMint: quoteClaimTokenMint,
        baseHedgeTokenMint: baseHedgeTokenMint,
        quoteHedgeTokenMint: quoteHedgeTokenMint,
        baseHedgeVault: baseHedgeVault,
        quoteHedgeVault: quoteHedgeVault,
        baseReserveVault: baseReserveVault,
        quoteReserveVault: quoteReserveVault,
        baseCollateralVault: baseCollateralVault,
        quoteCollateralVault: quoteCollateralVault,
        baseInsuranceVault: baseInsuranceVault,
        quoteInsuranceVault: quoteInsuranceVault,
        baseFeeVault: baseFeeVault,
        quoteFeeVault: quoteFeeVault,
        baseStakeVault: baseStakeVault,
        quoteStakeVault: quoteStakeVault,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .signers([payer])
      .rpc();

    return {
      signature,
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      quoteClaimTokenMint,
      baseHedgeTokenMint,
      quoteHedgeTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseCollateralVault,
      quoteCollateralVault,
      baseInsuranceVault,
      quoteInsuranceVault,
      baseFeeVault,
      quoteFeeVault,
      baseHedgeVault,
      quoteHedgeVault,
      baseStakeVault,
      quoteStakeVault,
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
    const [baseMint, quoteMint] = orderedMints(transferFeeMint, vanillaMint);
    const transferFeeSideIndex = baseMint.equals(transferFeeMint) ? 0 : 1;
    const paramsHash = Buffer.alloc(32, 8);
    const [market] = PublicKey.findProgramAddressSync(
      [Buffer.from("market_v2"), baseMint.toBuffer(), quoteMint.toBuffer(), paramsHash],
      OMNIPAIR_V2_PROGRAM_ID
    );
    const [eventAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      OMNIPAIR_V2_PROGRAM_ID
    );
    const baseClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
    const quoteClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
    const baseHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
    const quoteHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
    const baseHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), baseClaimTokenMint.toBuffer());
    const quoteHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), quoteClaimTokenMint.toBuffer());
    const baseReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), baseMint.toBuffer());
    const quoteReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), quoteMint.toBuffer());
    const baseCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), baseMint.toBuffer());
    const quoteCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), quoteMint.toBuffer());
    const baseInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), baseMint.toBuffer());
    const quoteInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), quoteMint.toBuffer());
    const baseFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), baseMint.toBuffer());
    const quoteFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), quoteMint.toBuffer());
    const baseStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), baseClaimTokenMint.toBuffer());
    const quoteStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), quoteClaimTokenMint.toBuffer());

    await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config: marketConfig(),
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        baseMint: baseMint,
        quoteMint: quoteMint,
        market,
        baseClaimTokenMint: baseClaimTokenMint,
        quoteClaimTokenMint: quoteClaimTokenMint,
        baseHedgeTokenMint: baseHedgeTokenMint,
        quoteHedgeTokenMint: quoteHedgeTokenMint,
        baseHedgeVault: baseHedgeVault,
        quoteHedgeVault: quoteHedgeVault,
        baseReserveVault: baseReserveVault,
        quoteReserveVault: quoteReserveVault,
        baseCollateralVault: baseCollateralVault,
        quoteCollateralVault: quoteCollateralVault,
        baseInsuranceVault: baseInsuranceVault,
        quoteInsuranceVault: quoteInsuranceVault,
        baseFeeVault: baseFeeVault,
        quoteFeeVault: quoteFeeVault,
        baseStakeVault: baseStakeVault,
        quoteStakeVault: quoteStakeVault,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .signers([payer])
      .rpc();

    return {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      quoteClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseCollateralVault,
      quoteCollateralVault,
      baseFeeVault,
      quoteFeeVault,
      eventAuthority,
      transferFeeMint,
      transferFeeSideIndex,
    };
  }

  async function addLiquiditySide(
    fixture,
    marketAsset,
    assetMint,
    claimTokenMint,
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
      .addLiquidity({
        marketAsset: normalizeMarketAsset(marketAsset),
        depositAmount: new BN(depositAmount),
        minClaimAmount: new BN(minClaimAmount),
        maxBufferAmount: new BN(maxBufferAmount),
      })
      .accounts({
        market: fixture.market,
        owner: owner.publicKey,
        assetMint,
        claimTokenMint,
        reserveVault,
        ownerAssetAccount,
        ownerClaimAccount,
        stakePosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([owner])
      .rpc();

    return stakePosition;
  }

  async function fetchStakePosition(stakePosition: PublicKey) {
    const account = await connection.getAccountInfo(stakePosition);
    expect(account).to.not.equal(null);
    return accountCoder.decode("StakePosition", account!.data) as any;
  }

  async function fundTwoSidedMarket() {
    const fixture = await initializeMarketFixture();
    const ownerBaseAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseMint,
      payer.publicKey
    );
    const ownerQuoteAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteMint,
      payer.publicKey
    );
    const ownerBaseClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseClaimTokenMint,
      payer.publicKey
    );
    const ownerQuoteClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteClaimTokenMint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.baseMint,
      ownerBaseAccount,
      payer,
      2_000_000
    );
    await mintTo(
      connection as any,
      payer,
      fixture.quoteMint,
      ownerQuoteAccount,
      payer,
      2_000_000
    );

    const stake0Position = await addLiquiditySide(
      fixture,
      0,
      fixture.baseMint,
      fixture.baseClaimTokenMint,
      fixture.baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount
    );
    const stake1Position = await addLiquiditySide(
      fixture,
      1,
      fixture.quoteMint,
      fixture.quoteClaimTokenMint,
      fixture.quoteReserveVault,
      ownerQuoteAccount,
      ownerQuoteClaimAccount
    );

    return {
      ...fixture,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      ownerQuoteClaimAccount,
      stake0Position,
      stake1Position,
    };
  }

  async function fundTinyRoundingMarket() {
    const fixture = await initializeMarketFixture();
    const ownerBaseAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseMint,
      payer.publicKey
    );
    const ownerQuoteAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteMint,
      payer.publicKey
    );
    const ownerBaseClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseClaimTokenMint,
      payer.publicKey
    );
    const ownerQuoteClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteClaimTokenMint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.baseMint,
      ownerBaseAccount,
      payer,
      100
    );
    await mintTo(
      connection as any,
      payer,
      fixture.quoteMint,
      ownerQuoteAccount,
      payer,
      100
    );

    const stake0Position = await addLiquiditySide(
      fixture,
      0,
      fixture.baseMint,
      fixture.baseClaimTokenMint,
      fixture.baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      6,
      4,
      2
    );
    const stake1Position = await addLiquiditySide(
      fixture,
      1,
      fixture.quoteMint,
      fixture.quoteClaimTokenMint,
      fixture.quoteReserveVault,
      ownerQuoteAccount,
      ownerQuoteClaimAccount,
      6,
      4,
      2
    );

    return {
      ...fixture,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      ownerQuoteClaimAccount,
      stake0Position,
      stake1Position,
    };
  }

  async function fundRoundedBorrowMarket(rounds = 12) {
    const fixture = await initializeMarketFixture("canonical", syntheticLiquidityRiskConfig());
    const ownerBaseAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseMint,
      payer.publicKey
    );
    const ownerQuoteAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteMint,
      payer.publicKey
    );
    const ownerBaseClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseClaimTokenMint,
      payer.publicKey
    );
    const ownerQuoteClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteClaimTokenMint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.baseMint,
      ownerBaseAccount,
      payer,
      300
    );
    await mintTo(
      connection as any,
      payer,
      fixture.quoteMint,
      ownerQuoteAccount,
      payer,
      300
    );

    for (let i = 0; i < rounds; i++) {
      const lender = Keypair.generate();
      await connection.requestAirdrop(lender.publicKey, LAMPORTS_PER_SOL);
      const lenderAsset0Account = await createAccount(
        connection as any,
        payer,
        fixture.baseMint,
        lender.publicKey
      );
      const lenderAsset1Account = await createAccount(
        connection as any,
        payer,
        fixture.quoteMint,
        lender.publicKey
      );
      const lenderClaim0Account = await createAccount(
        connection as any,
        payer,
        fixture.baseClaimTokenMint,
        lender.publicKey
      );
      const lenderClaim1Account = await createAccount(
        connection as any,
        payer,
        fixture.quoteClaimTokenMint,
        lender.publicKey
      );

      await mintTo(
        connection as any,
        payer,
        fixture.baseMint,
        lenderAsset0Account,
        payer,
        26
      );
      await mintTo(
        connection as any,
        payer,
        fixture.quoteMint,
        lenderAsset1Account,
        payer,
        26
      );

      await addLiquiditySide(
        fixture,
        0,
        fixture.baseMint,
        fixture.baseClaimTokenMint,
        fixture.baseReserveVault,
        lenderAsset0Account,
        lenderClaim0Account,
        26,
        20,
        6,
        lender
      );
      await addLiquiditySide(
        fixture,
        1,
        fixture.quoteMint,
        fixture.quoteClaimTokenMint,
        fixture.quoteReserveVault,
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
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      ownerQuoteClaimAccount,
    };
  }

  it("rejects transfer-fee claim and hedge mints at market initialization", async () => {
    async function expectTransferFeeMintRejected(blockedMintKind: "claim" | "hedge", paramsSeed: number) {
      const mintA = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const mintB = await createMint(connection as any, payer, payer.publicKey, null, 6);
      const [baseMint, quoteMint] = orderedMints(mintA, mintB);
      const paramsHash = Buffer.alloc(32, paramsSeed);
      const [market] = PublicKey.findProgramAddressSync(
        [Buffer.from("market_v2"), baseMint.toBuffer(), quoteMint.toBuffer(), paramsHash],
        OMNIPAIR_V2_PROGRAM_ID
      );
      const [eventAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from("__event_authority")],
        OMNIPAIR_V2_PROGRAM_ID
      );

      const blockedMint = await createTransferFeeMint(6, 1_000, 1_000_000, market);
      const baseClaimTokenMint = blockedMintKind === "claim"
        ? blockedMint
        : await createMint(connection as any, payer, market, null, 6);
      const quoteClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
      const baseHedgeTokenMint = blockedMintKind === "hedge"
        ? blockedMint
        : await createMint(connection as any, payer, market, null, 6);
      const quoteHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
      const baseHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), baseClaimTokenMint.toBuffer());
      const quoteHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), quoteClaimTokenMint.toBuffer());
      const baseReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), baseMint.toBuffer());
      const quoteReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), quoteMint.toBuffer());
      const baseCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), baseMint.toBuffer());
      const quoteCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), quoteMint.toBuffer());
      const baseInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), baseMint.toBuffer());
      const quoteInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), quoteMint.toBuffer());
      const baseFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), baseMint.toBuffer());
      const quoteFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), quoteMint.toBuffer());
      const baseStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), baseClaimTokenMint.toBuffer());
      const quoteStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), quoteClaimTokenMint.toBuffer());

      await expectRejects(() =>
        program.methods
          .initialize({
            operator: payer.publicKey,
            manager: payer.publicKey,
            config: marketConfig(),
            paramsHash: [...paramsHash],
          })
          .accounts({
            payer: payer.publicKey,
            baseMint: baseMint,
            quoteMint: quoteMint,
            market,
            baseClaimTokenMint: baseClaimTokenMint,
            quoteClaimTokenMint: quoteClaimTokenMint,
            baseHedgeTokenMint: baseHedgeTokenMint,
            quoteHedgeTokenMint: quoteHedgeTokenMint,
            baseHedgeVault: baseHedgeVault,
            quoteHedgeVault: quoteHedgeVault,
            baseReserveVault: baseReserveVault,
            quoteReserveVault: quoteReserveVault,
            baseCollateralVault: baseCollateralVault,
            quoteCollateralVault: quoteCollateralVault,
            baseInsuranceVault: baseInsuranceVault,
            quoteInsuranceVault: quoteInsuranceVault,
            baseFeeVault: baseFeeVault,
            quoteFeeVault: quoteFeeVault,
            baseStakeVault: baseStakeVault,
            quoteStakeVault: quoteStakeVault,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            eventAuthority,
            program: OMNIPAIR_V2_PROGRAM_ID,
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
      const [baseMint, quoteMint] = orderedMints(mintA, mintB);
      const paramsHash = Buffer.alloc(32, paramsSeed);
      const [market] = PublicKey.findProgramAddressSync(
        [Buffer.from("market_v2"), baseMint.toBuffer(), quoteMint.toBuffer(), paramsHash],
        OMNIPAIR_V2_PROGRAM_ID
      );
      const [eventAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from("__event_authority")],
        OMNIPAIR_V2_PROGRAM_ID
      );
      const baseClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
      const quoteClaimTokenMint = await createMint(connection as any, payer, market, null, 6);
      const baseHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
      const quoteHedgeTokenMint = await createMint(connection as any, payer, market, null, 6);
      const baseHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), baseClaimTokenMint.toBuffer());
      const quoteHedgeVault = deriveAddress(Buffer.from("hedged"), market.toBuffer(), quoteClaimTokenMint.toBuffer());
      const baseReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), baseMint.toBuffer());
      const quoteReserveVault = deriveAddress(Buffer.from("market_reserve"), market.toBuffer(), quoteMint.toBuffer());
      const baseCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), baseMint.toBuffer());
      const quoteCollateralVault = deriveAddress(Buffer.from("market_collateral"), market.toBuffer(), quoteMint.toBuffer());
      const baseInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), baseMint.toBuffer());
      const quoteInsuranceVault = deriveAddress(Buffer.from("insurance"), market.toBuffer(), quoteMint.toBuffer());
      const baseFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), baseMint.toBuffer());
      const quoteFeeVault = deriveAddress(Buffer.from("market_fee"), market.toBuffer(), quoteMint.toBuffer());
      const baseStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), baseClaimTokenMint.toBuffer());
      const quoteStakeVault = deriveAddress(Buffer.from("market_stake"), market.toBuffer(), quoteClaimTokenMint.toBuffer());

      await expectRejects(() =>
        program.methods
          .initialize({
            operator,
            manager,
            config: marketConfig(),
            paramsHash: [...paramsHash],
          })
          .accounts({
            payer: payer.publicKey,
            baseMint: baseMint,
            quoteMint: quoteMint,
            market,
            baseClaimTokenMint: baseClaimTokenMint,
            quoteClaimTokenMint: quoteClaimTokenMint,
            baseHedgeTokenMint: baseHedgeTokenMint,
            quoteHedgeTokenMint: quoteHedgeTokenMint,
            baseHedgeVault: baseHedgeVault,
            quoteHedgeVault: quoteHedgeVault,
            baseReserveVault: baseReserveVault,
            quoteReserveVault: quoteReserveVault,
            baseCollateralVault: baseCollateralVault,
            quoteCollateralVault: quoteCollateralVault,
            baseInsuranceVault: baseInsuranceVault,
            quoteInsuranceVault: quoteInsuranceVault,
            baseFeeVault: baseFeeVault,
            quoteFeeVault: quoteFeeVault,
            baseStakeVault: baseStakeVault,
            quoteStakeVault: quoteStakeVault,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            eventAuthority,
            program: OMNIPAIR_V2_PROGRAM_ID,
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
    trackInstruction("initialize", "initializes a market account");

    const {
      market,
      baseReserveVault,
      quoteReserveVault,
      baseHedgeVault,
      quoteHedgeVault,
      baseStakeVault,
      quoteStakeVault,
    } = await initializeMarketFixture();

    const marketAccount = await connection.getAccountInfo(market);
    expect(marketAccount).to.not.equal(null);
    expect(marketAccount.owner.toString()).to.equal(OMNIPAIR_V2_PROGRAM_ID.toString());

    for (const vault of [baseReserveVault, quoteReserveVault, baseHedgeVault, quoteHedgeVault, baseStakeVault, quoteStakeVault]) {
      const vaultAccount = await connection.getAccountInfo(vault);
      expect(vaultAccount).to.not.equal(null);
      expect(vaultAccount.owner.toString()).to.equal(TOKEN_PROGRAM_ID.toString());
    }
  });

  it("derives V2 market addresses through public SDK helpers", async () => {
    const fixture = await initializeMarketFixture();
    const paramsHash = Buffer.alloc(32, 7);
    const owner = payer.publicKey;

    expect(deriveMarketAddress(fixture.baseMint, fixture.quoteMint, paramsHash)[0].toString()).to.equal(
      fixture.market.toString()
    );
    expect(deriveMarketReserveVaultAddress(fixture.market, fixture.baseMint)[0].toString()).to.equal(
      fixture.baseReserveVault.toString()
    );
    expect(deriveMarketReserveVaultAddress(fixture.market, fixture.quoteMint)[0].toString()).to.equal(
      fixture.quoteReserveVault.toString()
    );
    expect(deriveMarketCollateralVaultAddress(fixture.market, fixture.baseMint)[0].toString()).to.equal(
      fixture.baseCollateralVault.toString()
    );
    expect(deriveMarketCollateralVaultAddress(fixture.market, fixture.quoteMint)[0].toString()).to.equal(
      fixture.quoteCollateralVault.toString()
    );
    expect(deriveMarketFeeVaultAddress(fixture.market, fixture.baseMint)[0].toString()).to.equal(
      fixture.baseFeeVault.toString()
    );
    expect(deriveMarketFeeVaultAddress(fixture.market, fixture.quoteMint)[0].toString()).to.equal(
      fixture.quoteFeeVault.toString()
    );
    expect(deriveMarketStakeVaultAddress(fixture.market, fixture.baseClaimTokenMint)[0].toString()).to.equal(
      fixture.baseStakeVault.toString()
    );
    expect(deriveMarketStakeVaultAddress(fixture.market, fixture.quoteClaimTokenMint)[0].toString()).to.equal(
      fixture.quoteStakeVault.toString()
    );
    expect(deriveHedgeVaultAddress(fixture.market, fixture.baseClaimTokenMint)[0].toString()).to.equal(
      fixture.baseHedgeVault.toString()
    );
    expect(deriveHedgeVaultAddress(fixture.market, fixture.quoteClaimTokenMint)[0].toString()).to.equal(
      fixture.quoteHedgeVault.toString()
    );
    expect(deriveInsuranceReserveAddress(fixture.market, fixture.baseMint)[0].toString()).to.equal(
      fixture.baseInsuranceVault.toString()
    );
    expect(deriveInsuranceReserveAddress(fixture.market, fixture.quoteMint)[0].toString()).to.equal(
      fixture.quoteInsuranceVault.toString()
    );
    expect(deriveStakePositionAddress(fixture.market, owner, fixture.baseMint)[0].toString()).to.equal(
      deriveAddress(
        Buffer.from("stake"),
        fixture.market.toBuffer(),
        owner.toBuffer(),
        fixture.baseMint.toBuffer()
      ).toString()
    );
    expect(deriveMarginPositionAddress(fixture.market, owner)[0].toString()).to.equal(
      deriveAddress(Buffer.from("margin"), fixture.market.toBuffer(), owner.toBuffer()).toString()
    );
    expect(deriveHedgePositionAddress(fixture.market, owner, fixture.baseMint)[0].toString()).to.equal(
      deriveAddress(
        Buffer.from("hedge_position"),
        fixture.market.toBuffer(),
        owner.toBuffer(),
        fixture.baseMint.toBuffer()
      ).toString()
    );
  });

  it("preserves creator-chosen non-canonical base and quote order", async () => {
    const fixture = await initializeMarketFixture("reversed");
    expect(Buffer.compare(fixture.baseMint.toBuffer(), fixture.quoteMint.toBuffer())).to.equal(1);

    const marketAccount = await connection.getAccountInfo(fixture.market);
    expect(marketAccount).to.not.equal(null);

    const events = decodeCpiEvents(svm, fixture.signature);
    const marketCreated = events.find((event) => event.name === "MarketCreated");
    expect(marketCreated).to.not.equal(undefined);
    expect(marketCreated.data.base_mint.toString()).to.equal(fixture.baseMint.toString());
    expect(marketCreated.data.quote_mint.toString()).to.equal(fixture.quoteMint.toString());
  });

  it("updates market config and enforces reduce-only mode", async () => {
    trackInstruction("updateConfig", "updates market buffer ratio");
    trackInstruction("setReduceOnly", "blocks risk-increasing reserve deposits");

    const fixture = await initializeMarketFixture();
    const ownerBaseAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseMint,
      payer.publicKey
    );
    const ownerBaseClaimAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseClaimTokenMint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      fixture.baseMint,
      ownerBaseAccount,
      payer,
      2_000_000
    );

    const config = marketConfig();
    config.bufferRatioBps = 1_000;
    const updateConfigSignature = await program.methods
      .updateConfig({ config })
      .accounts({
        market: fixture.market,
        operator: payer.publicKey,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const updateConfigEvents = decodeCpiEvents(svm, updateConfigSignature);
    const updateConfigEventNames = updateConfigEvents.map((event) => event.name);
    expect(updateConfigEventNames).to.include("MarketUpdated");
    expect(updateConfigEventNames).to.include("MarketHealthUpdated");
    const configHealthEvent = updateConfigEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(configHealthEvent.data.market.toString()).to.equal(fixture.market.toString());
    expect(configHealthEvent.data.effective_base_debt_nad.toString()).to.equal("0");

    const stake0Position = await addLiquiditySide(
      fixture,
      0,
      fixture.baseMint,
      fixture.baseClaimTokenMint,
      fixture.baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      1_000_000,
      900_000,
      100_000
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(900_000)
    );

    await program.methods
      .setReduceOnly({ reduceOnly: true })
      .accounts({
        market: fixture.market,
        authority: payer.publicKey,
        eventAuthority: fixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .addLiquidity({
          marketAsset: { base: {} },
          depositAmount: new BN(1_000),
          minClaimAmount: new BN(900),
          maxBufferAmount: new BN(100),
        })
        .accounts({
          market: fixture.market,
          owner: payer.publicKey,
          assetMint: fixture.baseMint,
          claimTokenMint: fixture.baseClaimTokenMint,
          reserveVault: fixture.baseReserveVault,
          ownerAssetAccount: ownerBaseAccount,
          ownerClaimAccount: ownerBaseClaimAccount,
          stakePosition: stake0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority: fixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
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
        .updateConfig({ config: softBorrowConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const unsafeHealthConfig = marketConfig();
    unsafeHealthConfig.recognizedCollateralCapBps = 10_500;
    unsafeHealthConfig.marketHealthMinBps = 11_000;
    await expectRejects(() =>
      program.methods
        .updateConfig({ config: unsafeHealthConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const validConfig = marketConfig();
    validConfig.swapFeeBps = 42;
    await program.methods
      .updateConfig({ config: validConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
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
        .updateConfig({ config })
        .accounts({
          market,
          operator: impostor.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );

    await expectRejects(() =>
      program.methods
        .setReduceOnly({ reduceOnly: true })
        .accounts({
          market,
          authority: impostor.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );
  });

  it("locks buffer-ratio updates while market stake is active", async () => {
    const {
      baseMint,
      baseClaimTokenMint,
      market,
      baseStakeVault,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTwoSidedMarket();

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(400_000),
        bufferShareAmount: new BN(100_000),
        minActiveStakeUnits: new BN(500_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const lockedConfig = marketConfig();
    lockedConfig.bufferRatioBps = 1_500;
    await expectRejects(() =>
      program.methods
        .updateConfig({ config: lockedConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(400_000)
    );
    expect((await getAccount(connection as any, baseStakeVault)).amount).to.equal(
      BigInt(400_000)
    );
  });

  it("locks buffer-ratio updates while staker fee liability is outstanding", async () => {
    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      baseStakeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    config.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(4),
        bufferShareAmount: new BN(1),
        minActiveStakeUnits: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(12),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .unstake({
        marketAsset: { base: {} },
        claimAmount: new BN(4),
        bufferShareAmount: new BN(1),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const lockedConfig = marketConfig();
    lockedConfig.swapFeeBps = config.swapFeeBps;
    lockedConfig.bufferRatioBps = 1_500;
    await expectRejects(() =>
      program.methods
        .updateConfig({ config: lockedConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(4)
    );
    expect((await getAccount(connection as any, baseStakeVault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(10));
  });

  it("locks buffer-ratio updates while no-stake LP fees are carried forward", async () => {
    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    config.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(12),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const lockedConfig = marketConfig();
    lockedConfig.swapFeeBps = config.swapFeeBps;
    lockedConfig.bufferRatioBps = 1_500;
    await expectRejects(() =>
      program.methods
        .updateConfig({ config: lockedConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(10));
  });

  it("rejects buffer-ratio updates when the recomputed floor is uncovered", async () => {
    const {
      baseMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundTwoSidedMarket();

    const uncoveredConfig = marketConfig();
    uncoveredConfig.bufferRatioBps = 2_500;
    await expectRejects(() =>
      program.methods
        .updateConfig({ config: uncoveredConfig })
        .accounts({
          market,
          operator: payer.publicKey,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await addLiquiditySide(
      { market, eventAuthority },
      0,
      baseMint,
      baseClaimTokenMint,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      1_000,
      800,
      200
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(800_800)
    );
  });

  it("adds liquidity inventory and redeems fixed principal", async () => {
    trackInstruction("addLiquidity", "adds liquidity inventory");
    trackInstruction("removeLiquidity", "redeems fixed principal");

    const {
      baseMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTwoSidedMarket();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(1_000_000)
    );
    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(800_000)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(1_000_000)
    );
    const stakePositionBefore = await fetchStakePosition(stake0Position);
    expect(stakePositionBefore.available_buffer_share_amount.toString()).to.equal("200000");
    expect(stakePositionBefore.staked_buffer_share_amount.toString()).to.equal("0");

    await expectRejects(() =>
      program.methods
        .addLiquidity({
          marketAsset: { base: {} },
          depositAmount: new BN(900_000),
          minClaimAmount: new BN(720_000),
          maxBufferAmount: new BN(180_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          reserveVault: baseReserveVault,
          ownerAssetAccount: ownerBaseAccount,
          ownerClaimAccount: ownerBaseClaimAccount,
          stakePosition: stake0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );
    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(1_000_000)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(1_000_000)
    );

    const removeLiquiditySignature = await program.methods
      .removeLiquidity({
        marketAsset: { base: {} },
        claimAmount: new BN(80_000),
        minAssetAmountOut: new BN(80_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        reserveVault: baseReserveVault,
        ownerAssetAccount: ownerBaseAccount,
        ownerClaimAccount: ownerBaseClaimAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const removeLiquidityEvents = decodeCpiEvents(svm, removeLiquiditySignature);
    const removeLiquidityEventNames = removeLiquidityEvents.map((event) => event.name);
    expect(removeLiquidityEventNames).to.include("LiquidityRemoved");
    expect(removeLiquidityEventNames).to.include("MarketHealthUpdated");
    const removeLiquidityHealthEvent = removeLiquidityEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(removeLiquidityHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(1_080_000)
    );
    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(720_000)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(920_000)
    );
    const stakePositionAfter = await fetchStakePosition(stake0Position);
    expect(stakePositionAfter.available_buffer_share_amount.toString()).to.equal("200000");
    expect(stakePositionAfter.staked_buffer_share_amount.toString()).to.equal("0");
  });

  it("accounts for Token-2022 transfer fees with market inventory credits", async () => {
    const fixture = await initializeTransferFeeMarketFixture();
    const tokenProgramForMint = (mint: PublicKey) =>
      mint.equals(fixture.transferFeeMint) ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      quoteClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseCollateralVault,
      quoteCollateralVault,
      baseFeeVault,
      quoteFeeVault,
      eventAuthority,
      transferFeeSideIndex,
    } = fixture;
    const ownerBaseAccount = await createAccount(
      connection as any,
      payer,
      baseMint,
      payer.publicKey,
      undefined,
      undefined,
      tokenProgramForMint(baseMint)
    );
    const ownerQuoteAccount = await createAccount(
      connection as any,
      payer,
      quoteMint,
      payer.publicKey,
      undefined,
      undefined,
      tokenProgramForMint(quoteMint)
    );
    const ownerBaseClaimAccount = await createAccount(
      connection as any,
      payer,
      baseClaimTokenMint,
      payer.publicKey
    );
    const ownerQuoteClaimAccount = await createAccount(
      connection as any,
      payer,
      quoteClaimTokenMint,
      payer.publicKey
    );

    await mintTo(
      connection as any,
      payer,
      baseMint,
      ownerBaseAccount,
      payer,
      2_000,
      [],
      undefined,
      tokenProgramForMint(baseMint)
    );
    await mintTo(
      connection as any,
      payer,
      quoteMint,
      ownerQuoteAccount,
      payer,
      2_000,
      [],
      undefined,
      tokenProgramForMint(quoteMint)
    );

    const transferFeeSide = transferFeeSideIndex === 0
      ? {
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          reserveVault: baseReserveVault,
          ownerAssetAccount: ownerBaseAccount,
          ownerClaimAccount: ownerBaseClaimAccount,
          collateralVault: quoteCollateralVault,
          collateralAssetMint: quoteMint,
          collateralOwnerAccount: ownerQuoteAccount,
          borrowAsset: { base: {} },
        }
      : {
          assetMint: quoteMint,
          claimTokenMint: quoteClaimTokenMint,
          reserveVault: quoteReserveVault,
          ownerAssetAccount: ownerQuoteAccount,
          ownerClaimAccount: ownerQuoteClaimAccount,
          collateralVault: baseCollateralVault,
          collateralAssetMint: baseMint,
          collateralOwnerAccount: ownerBaseAccount,
          borrowAsset: { quote: {} },
        };
    const vanillaSide = transferFeeSideIndex === 0
      ? {
          marketAsset: { quote: {} },
          assetMint: quoteMint,
          claimTokenMint: quoteClaimTokenMint,
          reserveVault: quoteReserveVault,
          feeVault: quoteFeeVault,
          ownerAssetAccount: ownerQuoteAccount,
          ownerClaimAccount: ownerQuoteClaimAccount,
        }
      : {
          marketAsset: { base: {} },
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          reserveVault: baseReserveVault,
          feeVault: baseFeeVault,
          ownerAssetAccount: ownerBaseAccount,
          ownerClaimAccount: ownerBaseClaimAccount,
        };

    await addLiquiditySide(
      fixture,
      transferFeeSideIndex,
      transferFeeSide.assetMint,
      transferFeeSide.claimTokenMint,
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
        transferFeeSide.claimTokenMint,
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
      await addLiquiditySide(
        fixture,
        transferFeeSideIndex,
        transferFeeSide.assetMint,
        transferFeeSide.claimTokenMint,
        transferFeeSide.reserveVault,
        lenderAssetAccount,
        lenderClaimAccount,
        29,
        20,
        6,
        lender
      );
    }
    await addLiquiditySide(
      fixture,
      vanillaSide.marketAsset,
      vanillaSide.assetMint,
      vanillaSide.claimTokenMint,
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
        .removeLiquidity({
          marketAsset: marketAssetFromIndex(transferFeeSideIndex),
          claimAmount: new BN(5),
          minAssetAmountOut: new BN(5),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: transferFeeSide.assetMint,
          claimTokenMint: transferFeeSide.claimTokenMint,
          reserveVault: transferFeeSide.reserveVault,
          ownerAssetAccount: transferFeeSide.ownerAssetAccount,
          ownerClaimAccount: transferFeeSide.ownerClaimAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
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
      .removeLiquidity({
        marketAsset: marketAssetFromIndex(transferFeeSideIndex),
        claimAmount: new BN(5),
        minAssetAmountOut: new BN(4),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: transferFeeSide.assetMint,
        claimTokenMint: transferFeeSide.claimTokenMint,
        reserveVault: transferFeeSide.reserveVault,
        ownerAssetAccount: transferFeeSide.ownerAssetAccount,
        ownerClaimAccount: transferFeeSide.ownerClaimAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
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
        .swap({
          assetIn: vanillaSide.marketAsset,
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
          program: OMNIPAIR_V2_PROGRAM_ID,
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
      .swap({
        assetIn: vanillaSide.marketAsset,
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
        program: OMNIPAIR_V2_PROGRAM_ID,
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
        marketAsset: oppositeMarketAssetFromIndex(transferFeeSideIndex),
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
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: transferFeeSide.borrowAsset,
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
          program: OMNIPAIR_V2_PROGRAM_ID,
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
      .borrow({
        borrowAsset: transferFeeSide.borrowAsset,
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
        program: OMNIPAIR_V2_PROGRAM_ID,
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
    advanceRiskEmaWindow();
    const borrowLimitConfig = syntheticLiquidityRiskConfig();
    borrowLimitConfig.maxDailyBorrowBps = 300;
    await program.methods
      .updateConfig({ config: borrowLimitConfig })
      .accounts({
        market: borrowFixture.market,
        operator: payer.publicKey,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
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
        marketAsset: { quote: {} },
        depositAmount: new BN(200),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        assetMint: borrowFixture.quoteMint,
        collateralVault: borrowFixture.quoteCollateralVault,
        ownerAssetAccount: borrowFixture.ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(9),
        minDebtAmountOut: new BN(9),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        debtAssetMint: borrowFixture.baseMint,
        collateralAssetMint: borrowFixture.quoteMint,
        reserveVault: borrowFixture.baseReserveVault,
        ownerDebtAccount: borrowFixture.ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: { base: {} },
          borrowAmount: new BN(1),
          minDebtAmountOut: new BN(1),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market: borrowFixture.market,
          owner: payer.publicKey,
          debtAssetMint: borrowFixture.baseMint,
          collateralAssetMint: borrowFixture.quoteMint,
          reserveVault: borrowFixture.baseReserveVault,
          ownerDebtAccount: borrowFixture.ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: borrowFixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    const redeemFixture = await fundTwoSidedMarket();
    const redeemLimitConfig = marketConfig();
    redeemLimitConfig.maxDailyWithdrawBps = 1;
    await program.methods
      .updateConfig({ config: redeemLimitConfig })
      .accounts({
        market: redeemFixture.market,
        operator: payer.publicKey,
        eventAuthority: redeemFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .removeLiquidity({
        marketAsset: { base: {} },
        claimAmount: new BN(100),
        minAssetAmountOut: new BN(100),
      })
      .accounts({
        market: redeemFixture.market,
        owner: payer.publicKey,
        assetMint: redeemFixture.baseMint,
        claimTokenMint: redeemFixture.baseClaimTokenMint,
        reserveVault: redeemFixture.baseReserveVault,
        ownerAssetAccount: redeemFixture.ownerBaseAccount,
        ownerClaimAccount: redeemFixture.ownerBaseClaimAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: redeemFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .removeLiquidity({
          marketAsset: { base: {} },
          claimAmount: new BN(1),
          minAssetAmountOut: new BN(1),
        })
        .accounts({
          market: redeemFixture.market,
          owner: payer.publicKey,
          assetMint: redeemFixture.baseMint,
          claimTokenMint: redeemFixture.baseClaimTokenMint,
          reserveVault: redeemFixture.baseReserveVault,
          ownerAssetAccount: redeemFixture.ownerBaseAccount,
          ownerClaimAccount: redeemFixture.ownerBaseClaimAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: redeemFixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const collateralWithdrawFixture = await fundRoundedBorrowMarket();
    advanceRiskEmaWindow();
    const collateralWithdrawLimitConfig = syntheticLiquidityRiskConfig();
    collateralWithdrawLimitConfig.maxDailyWithdrawBps = 320;
    await program.methods
      .updateConfig({ config: collateralWithdrawLimitConfig })
      .accounts({
        market: collateralWithdrawFixture.market,
        operator: payer.publicKey,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
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
        marketAsset: { quote: {} },
        depositAmount: new BN(20),
      })
      .accounts({
        market: collateralWithdrawFixture.market,
        owner: payer.publicKey,
        assetMint: collateralWithdrawFixture.quoteMint,
        collateralVault: collateralWithdrawFixture.quoteCollateralVault,
        ownerAssetAccount: collateralWithdrawFixture.ownerQuoteAccount,
        marginPosition: collateralWithdrawMarginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .withdrawCollateral({
        marketAsset: { quote: {} },
        withdrawAmount: new BN(9),
        minAssetAmountOut: new BN(9),
      })
      .accounts({
        market: collateralWithdrawFixture.market,
        owner: payer.publicKey,
        assetMint: collateralWithdrawFixture.quoteMint,
        collateralVault: collateralWithdrawFixture.quoteCollateralVault,
        ownerAssetAccount: collateralWithdrawFixture.ownerQuoteAccount,
        marginPosition: collateralWithdrawMarginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: collateralWithdrawFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await expectRejects(() =>
      program.methods
        .withdrawCollateral({
          marketAsset: { quote: {} },
          withdrawAmount: new BN(1),
          minAssetAmountOut: new BN(1),
        })
        .accounts({
          market: collateralWithdrawFixture.market,
          owner: payer.publicKey,
          assetMint: collateralWithdrawFixture.quoteMint,
          collateralVault: collateralWithdrawFixture.quoteCollateralVault,
          ownerAssetAccount: collateralWithdrawFixture.ownerQuoteAccount,
          marginPosition: collateralWithdrawMarginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: collateralWithdrawFixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );
  });

  it("swaps against market reserve floor excess", async () => {
    trackInstruction("swap", "swaps against rounded market reserve excess");

    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const relaxedRiskConfig = marketConfig();
    relaxedRiskConfig.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config: relaxedRiskConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const swapSignature = await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(3),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const swapEvents = decodeCpiEvents(svm, swapSignature);
    const swapEventNames = swapEvents.map((event) => event.name);
    expect(swapEventNames).to.include("SwapExecuted");
    expect(swapEventNames).to.include("MarketHealthUpdated");
    const swapHealthEvent = swapEvents.find((event) => event.name === "MarketHealthUpdated");
    expect(swapHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(91)
    );
    expect((await getAccount(connection as any, ownerQuoteAccount)).amount).to.equal(
      BigInt(95)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(8)
    );
    expect((await getAccount(connection as any, quoteReserveVault)).amount).to.equal(
      BigInt(5)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(1));
  });

  it("blocks market swaps and borrows in reduce-only mode", async () => {
    const swapFixture = await fundTinyRoundingMarket();

    await program.methods
      .setReduceOnly({ reduceOnly: true })
      .accounts({
        market: swapFixture.market,
        authority: payer.publicKey,
        eventAuthority: swapFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .swap({
          assetIn: { base: {} },
          exactAssetIn: new BN(3),
          minAssetOut: new BN(1),
        })
        .accounts({
          market: swapFixture.market,
          trader: payer.publicKey,
          assetInMint: swapFixture.baseMint,
          assetOutMint: swapFixture.quoteMint,
          reserveInVault: swapFixture.baseReserveVault,
          reserveOutVault: swapFixture.quoteReserveVault,
          feeInVault: swapFixture.baseFeeVault,
          traderAssetInAccount: swapFixture.ownerBaseAccount,
          traderAssetOutAccount: swapFixture.ownerQuoteAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: swapFixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
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
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market: borrowFixture.market,
        owner: payer.publicKey,
        assetMint: borrowFixture.quoteMint,
        collateralVault: borrowFixture.quoteCollateralVault,
        ownerAssetAccount: borrowFixture.ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .setReduceOnly({ reduceOnly: true })
      .accounts({
        market: borrowFixture.market,
        authority: payer.publicKey,
        eventAuthority: borrowFixture.eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: { base: {} },
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(5),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market: borrowFixture.market,
          owner: payer.publicKey,
          debtAssetMint: borrowFixture.baseMint,
          collateralAssetMint: borrowFixture.quoteMint,
          reserveVault: borrowFixture.baseReserveVault,
          ownerDebtAccount: borrowFixture.ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority: borrowFixture.eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );
  });

  it("claims staker and operator market fees", async () => {
    trackInstruction("claimFees", "claims non-compounding staker fees");
    trackInstruction("claimMarketFees", "claims operator and protocol market fee liabilities");

    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      baseStakeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    config.protocolFeeBps = 2_000;
    config.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(4),
        bufferShareAmount: new BN(1),
        minActiveStakeUnits: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(12),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(10));

    const claimFeesSignature = await program.methods
      .claimFees({
        marketAsset: { base: {} },
        minFeeAmount: new BN(7),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        feeVault: baseFeeVault,
        ownerFeeAccount: ownerBaseAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const claimFeesEvents = decodeCpiEvents(svm, claimFeesSignature);
    const claimFeesEventNames = claimFeesEvents.map((event) => event.name);
    expect(claimFeesEventNames).to.include("MarketFeesClaimed");
    expect(claimFeesEventNames).to.include("MarketHealthUpdated");
    const claimFeesHealthEvent = claimFeesEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(claimFeesHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(89)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(3));

    const impostor = Keypair.generate();
    await connection.requestAirdrop(impostor.publicKey, LAMPORTS_PER_SOL);
    const impostorBaseAccount = await createAccount(
      connection as any,
      payer,
      baseMint,
      impostor.publicKey
    );
    await expectRejects(() =>
      program.methods
        .claimMarketFees({
          marketAsset: { base: {} },
          claimKind: { operator: {} },
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          feeAuthority: impostor.publicKey,
          assetMint: baseMint,
          feeVault: baseFeeVault,
          recipientFeeAccount: impostorBaseAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([impostor])
        .rpc()
    );

    expect((await getAccount(connection as any, impostorBaseAccount)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(3));

    const operatorFeeSignature = await program.methods
      .claimMarketFees({
        marketAsset: { base: {} },
        claimKind: { operator: {} },
        minFeeAmount: new BN(1),
      })
      .accounts({
        market,
        feeAuthority: payer.publicKey,
        assetMint: baseMint,
        feeVault: baseFeeVault,
        recipientFeeAccount: ownerBaseAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const operatorFeeEvents = decodeCpiEvents(svm, operatorFeeSignature);
    const operatorFeeEventNames = operatorFeeEvents.map((event) => event.name);
    expect(operatorFeeEventNames).to.include("MarketFeeLiabilityClaimed");
    expect(operatorFeeEventNames).to.include("MarketHealthUpdated");
    const operatorFeeHealthEvent = operatorFeeEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(operatorFeeHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(90)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(2));

    await program.methods
      .claimMarketFees({
        marketAsset: { base: {} },
        claimKind: { protocol: {} },
        minFeeAmount: new BN(2),
      })
      .accounts({
        market,
        feeAuthority: payer.publicKey,
        assetMint: baseMint,
        feeVault: baseFeeVault,
        recipientFeeAccount: ownerBaseAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(92)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(0));
  });

  it("carries no-stake LP fees into the next active market stake", async () => {
    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      baseStakeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    config.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(12),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(10));

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(4),
        bufferShareAmount: new BN(1),
        minActiveStakeUnits: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .claimFees({
        marketAsset: { base: {} },
        minFeeAmount: new BN(9),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        feeVault: baseFeeVault,
        ownerFeeAccount: ownerBaseAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(91)
    );
    expect((await getAccount(connection as any, baseFeeVault)).amount).to.equal(BigInt(1));
  });

  it("blocks fee claims when spot diverges from cached EMA", async () => {
    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      baseStakeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTinyRoundingMarket();

    const config = marketConfig();
    config.swapFeeBps = 8_000;
    config.spotEmaDivergenceBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(4),
        bufferShareAmount: new BN(1),
        minActiveStakeUnits: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(12),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await addLiquiditySide(
      { market, eventAuthority },
      0,
      baseMint,
      baseClaimTokenMint,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      2,
      1,
      1
    );

    const strictRiskConfig = marketConfig();
    strictRiskConfig.swapFeeBps = 8_000;
    await program.methods
      .updateConfig({ config: strictRiskConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .claimFees({
          marketAsset: { base: {} },
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          feeVault: baseFeeVault,
          ownerFeeAccount: ownerBaseAccount,
          stakePosition: stake0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await expectRejects(() =>
      program.methods
        .claimMarketFees({
          marketAsset: { base: {} },
          claimKind: { operator: {} },
          minFeeAmount: new BN(1),
        })
        .accounts({
          market,
          feeAuthority: payer.publicKey,
          assetMint: baseMint,
          feeVault: baseFeeVault,
          recipientFeeAccount: ownerBaseAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );
  });

  it("stakes and unstakes matched market claims and buffer shares", async () => {
    trackInstruction("stake", "stakes matched market claim and buffer shares");
    trackInstruction("unstake", "unstakes matched market claim and buffer shares");

    const {
      baseMint,
      baseClaimTokenMint,
      market,
      baseStakeVault,
      ownerBaseClaimAccount,
      stake0Position,
      eventAuthority,
    } = await fundTwoSidedMarket();

    await program.methods
      .stake({
        marketAsset: { base: {} },
        claimAmount: new BN(400_000),
        bufferShareAmount: new BN(100_000),
        minActiveStakeUnits: new BN(500_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(400_000)
    );
    expect((await getAccount(connection as any, baseStakeVault)).amount).to.equal(
      BigInt(400_000)
    );

    await program.methods
      .unstake({
        marketAsset: { base: {} },
        claimAmount: new BN(160_000),
        bufferShareAmount: new BN(40_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        stakeVault: baseStakeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        stakePosition: stake0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(560_000)
    );
    expect((await getAccount(connection as any, baseStakeVault)).amount).to.equal(
      BigInt(240_000)
    );
  });

  it("opens and closes hedged market claim wrappers", async () => {
    trackInstruction("openHedge", "wraps market claims into hedged claim tokens");
    trackInstruction("claimHedgeFees", "claims routed hedged market fees");
    trackInstruction("closeHedge", "unwraps hedged claim tokens into market claims");

    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      baseHedgeTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      baseFeeVault,
      baseHedgeVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundTinyRoundingMarket();
    const hedgeFeeConfig = marketConfig();
    hedgeFeeConfig.spotEmaDivergenceBps = 10_000;
    hedgeFeeConfig.kEmaDrawdownBps = 10_000;
    hedgeFeeConfig.effectiveDebtWeightMinBps = 0;
    hedgeFeeConfig.effectiveDebtGammaNad = new BN(0);
    await program.methods
      .updateConfig({ config: hedgeFeeConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const ownerHedge0Account = await createAccount(
      connection as any,
      payer,
      baseHedgeTokenMint,
      payer.publicKey
    );
    const hedge0Position = deriveAddress(
      Buffer.from("hedge_position"),
      market.toBuffer(),
      payer.publicKey.toBuffer(),
      baseMint.toBuffer()
    );

    const openHedgeSignature = await program.methods
      .openHedge({
        marketAsset: { base: {} },
        claimAmount: new BN(1),
        minHedgeAmount: new BN(1),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        hedgeTokenMint: baseHedgeTokenMint,
        hedgeVault: baseHedgeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        ownerHedgeAccount: ownerHedge0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const openHedgeEvents = decodeCpiEvents(svm, openHedgeSignature);
    const openHedgeEventNames = openHedgeEvents.map((event) => event.name);
    expect(openHedgeEventNames).to.include("MarketHedgeOpened");
    expect(openHedgeEventNames).to.include("MarketHealthUpdated");
    const openHedgeHealthEvent = openHedgeEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(openHedgeHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(3)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(1)
    );
    expect((await getAccount(connection as any, baseHedgeVault)).amount).to.equal(
      BigInt(1)
    );

    await program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(3),
        minAssetOut: new BN(1),
      })
      .accounts({
        market,
        trader: payer.publicKey,
        assetInMint: baseMint,
        assetOutMint: quoteMint,
        reserveInVault: baseReserveVault,
        reserveOutVault: quoteReserveVault,
        feeInVault: baseFeeVault,
        traderAssetInAccount: ownerBaseAccount,
        traderAssetOutAccount: ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const ownerAsset0BeforeHedgeFeeClaim = (await getAccount(
      connection as any,
      ownerBaseAccount
    )).amount;
    const claimHedgeFeesSignature = await program.methods
      .claimHedgeFees({
        marketAsset: { base: {} },
        minFeeAmount: new BN(1),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        feeVault: baseFeeVault,
        ownerFeeAccount: ownerBaseAccount,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const claimHedgeFeesEvents = decodeCpiEvents(svm, claimHedgeFeesSignature);
    const claimHedgeFeesEventNames = claimHedgeFeesEvents.map((event) => event.name);
    expect(claimHedgeFeesEventNames).to.include("MarketHedgeFeesClaimed");
    expect(claimHedgeFeesEventNames).to.include("MarketHealthUpdated");
    const claimHedgeFeesHealthEvent = claimHedgeFeesEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(claimHedgeFeesHealthEvent.data.market.toString()).to.equal(market.toString());
    expect(
      (await getAccount(connection as any, ownerBaseAccount)).amount >
        ownerAsset0BeforeHedgeFeeClaim
    ).to.equal(true);

    await program.methods
      .setReduceOnly({ reduceOnly: true })
      .accounts({
        market,
        authority: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .openHedge({
          marketAsset: { base: {} },
          claimAmount: new BN(1),
          minHedgeAmount: new BN(1),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          hedgeTokenMint: baseHedgeTokenMint,
          hedgeVault: baseHedgeVault,
          ownerClaimAccount: ownerBaseClaimAccount,
          ownerHedgeAccount: ownerHedge0Account,
          hedgePosition: hedge0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    const closeHedgeSignature = await program.methods
      .closeHedge({
        marketAsset: { base: {} },
        hedgeAmount: new BN(1),
        minClaimAmountOut: new BN(1),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        hedgeTokenMint: baseHedgeTokenMint,
        hedgeVault: baseHedgeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        ownerHedgeAccount: ownerHedge0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
    const closeHedgeEvents = decodeCpiEvents(svm, closeHedgeSignature);
    const closeHedgeEventNames = closeHedgeEvents.map((event) => event.name);
    expect(closeHedgeEventNames).to.include("MarketHedgeClosed");
    expect(closeHedgeEventNames).to.include("MarketHealthUpdated");
    const closeHedgeHealthEvent = closeHedgeEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(closeHedgeHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(4)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, baseHedgeVault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("blocks hedge opens that breach market health", async () => {
    const {
      baseMint,
      baseClaimTokenMint,
      baseHedgeTokenMint,
      market,
      baseHedgeVault,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundTinyRoundingMarket();
    const ownerHedge0Account = await createAccount(
      connection as any,
      payer,
      baseHedgeTokenMint,
      payer.publicKey
    );
    const hedge0Position = deriveAddress(
      Buffer.from("hedge_position"),
      market.toBuffer(),
      payer.publicKey.toBuffer(),
      baseMint.toBuffer()
    );
    const strictHedgeConfig = syntheticLiquidityRiskConfig();
    await program.methods
      .updateConfig({ config: strictHedgeConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .openHedge({
          marketAsset: { base: {} },
          claimAmount: new BN(1),
          minHedgeAmount: new BN(1),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          hedgeTokenMint: baseHedgeTokenMint,
          hedgeVault: baseHedgeVault,
          ownerClaimAccount: ownerBaseClaimAccount,
          ownerHedgeAccount: ownerHedge0Account,
          hedgePosition: hedge0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(4)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, baseHedgeVault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("blocks hedged market claim wrappers when disabled by config", async () => {
    const {
      baseMint,
      baseClaimTokenMint,
      baseHedgeTokenMint,
      market,
      baseHedgeVault,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundTwoSidedMarket();
    const ownerHedge0Account = await createAccount(
      connection as any,
      payer,
      baseHedgeTokenMint,
      payer.publicKey
    );
    const hedge0Position = deriveAddress(
      Buffer.from("hedge_position"),
      market.toBuffer(),
      payer.publicKey.toBuffer(),
      baseMint.toBuffer()
    );

    const config = marketConfig();
    config.hedgedLpEnabled = false;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .openHedge({
          marketAsset: { base: {} },
          claimAmount: new BN(50_000),
          minHedgeAmount: new BN(50_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          hedgeTokenMint: baseHedgeTokenMint,
          hedgeVault: baseHedgeVault,
          ownerClaimAccount: ownerBaseClaimAccount,
          ownerHedgeAccount: ownerHedge0Account,
          hedgePosition: hedge0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseClaimAccount)).amount).to.equal(
      BigInt(800_000)
    );
    expect((await getAccount(connection as any, ownerHedge0Account)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, baseHedgeVault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("blocks hedge closes when spot diverges from cached EMA", async () => {
    const {
      baseMint,
      baseClaimTokenMint,
      baseHedgeTokenMint,
      market,
      baseReserveVault,
      baseHedgeVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundTwoSidedMarket();
    const ownerHedge0Account = await createAccount(
      connection as any,
      payer,
      baseHedgeTokenMint,
      payer.publicKey
    );
    const hedge0Position = deriveAddress(
      Buffer.from("hedge_position"),
      market.toBuffer(),
      payer.publicKey.toBuffer(),
      baseMint.toBuffer()
    );
    const openSetupConfig = syntheticLiquidityRiskConfig();
    openSetupConfig.effectiveDebtWeightMinBps = 0;
    openSetupConfig.effectiveDebtGammaNad = new BN(0);
    await program.methods
      .updateConfig({ config: openSetupConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .openHedge({
        marketAsset: { base: {} },
        claimAmount: new BN(200_000),
        minHedgeAmount: new BN(200_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        claimTokenMint: baseClaimTokenMint,
        hedgeTokenMint: baseHedgeTokenMint,
        hedgeVault: baseHedgeVault,
        ownerClaimAccount: ownerBaseClaimAccount,
        ownerHedgeAccount: ownerHedge0Account,
        hedgePosition: hedge0Position,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .updateConfig({ config: syntheticLiquidityRiskConfig() })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await addLiquiditySide(
      { market, eventAuthority },
      0,
      baseMint,
      baseClaimTokenMint,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      500_000,
      400_000,
      100_000
    );

    await program.methods
      .updateConfig({ config: marketConfig() })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .closeHedge({
          marketAsset: { base: {} },
          hedgeAmount: new BN(50_000),
          minClaimAmountOut: new BN(50_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: baseMint,
          claimTokenMint: baseClaimTokenMint,
          hedgeTokenMint: baseHedgeTokenMint,
          hedgeVault: baseHedgeVault,
          ownerClaimAccount: ownerBaseClaimAccount,
          ownerHedgeAccount: ownerHedge0Account,
          hedgePosition: hedge0Position,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );
  });

  it("deposits collateral and funds market insurance", async () => {
    trackInstruction("depositCollateral", "deposits borrower collateral");
    trackInstruction("depositInsurance", "funds market insurance reserves");

    const {
      baseMint,
      quoteMint,
      market,
      quoteCollateralVault,
      baseInsuranceVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundTwoSidedMarket();

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(300_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition: deriveAddress(
          Buffer.from("margin"),
          market.toBuffer(),
          payer.publicKey.toBuffer()
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerQuoteAccount)).amount).to.equal(
      BigInt(700_000)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
      BigInt(300_000)
    );

    await program.methods
      .depositInsurance({
        marketAsset: { base: {} },
        depositAmount: new BN(125_000),
      })
      .accounts({
        market,
        sponsor: payer.publicKey,
        assetMint: baseMint,
        insuranceVault: baseInsuranceVault,
        sponsorAssetAccount: ownerBaseAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(875_000)
    );
    expect((await getAccount(connection as any, baseInsuranceVault)).amount).to.equal(
      BigInt(125_000)
    );
  });

  it("borrows and repays fixed market debt against recognized collateral", async () => {
    trackInstruction("borrow", "borrows fixed market debt");
    trackInstruction("repay", "repays fixed market debt");
    trackInstruction("withdrawCollateral", "withdraws idle borrower collateral");

    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    advanceRiskEmaWindow();
    const config = syntheticLiquidityRiskConfig();
    config.maxDailyWithdrawBps = 10_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: { base: {} },
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(6),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          ownerDebtAccount: ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(305)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(307)
    );

    await expectRejects(() =>
      program.methods
        .withdrawCollateral({
          marketAsset: { quote: {} },
          withdrawAmount: new BN(60),
          minAssetAmountOut: new BN(60),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          assetMint: quoteMint,
          collateralVault: quoteCollateralVault,
          ownerAssetAccount: ownerQuoteAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .rpc()
    );

    await program.methods
      .repay({
        repayAsset: { base: {} },
        repayAmount: new BN(5),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, ownerQuoteAccount)).amount).to.equal(
      BigInt(240)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
      BigInt(60)
    );

    const withdrawCollateralSignature = await program.methods
      .withdrawCollateral({
        marketAsset: { quote: {} },
        withdrawAmount: new BN(60),
        minAssetAmountOut: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();
    const withdrawCollateralEvents = decodeCpiEvents(svm, withdrawCollateralSignature);
    const withdrawCollateralEventNames = withdrawCollateralEvents.map((event) => event.name);
    expect(withdrawCollateralEventNames).to.include("MarketCollateralWithdrawn");
    expect(withdrawCollateralEventNames).to.include("MarketHealthUpdated");
    const withdrawCollateralHealthEvent = withdrawCollateralEvents.find(
      (event) => event.name === "MarketHealthUpdated"
    );
    expect(withdrawCollateralHealthEvent.data.market.toString()).to.equal(market.toString());

    expect((await getAccount(connection as any, ownerQuoteAccount)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
      BigInt(0)
    );
  });

  it("rejects fixed market borrows below required position health", async () => {
    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: { base: {} },
          borrowAmount: new BN(12),
          minDebtAmountOut: new BN(12),
          minHealthBps: new BN(20_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          ownerDebtAccount: ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(300)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(312)
    );
  });

  it("rejects fixed market borrows against same-side idle collateral", async () => {
    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      baseCollateralVault,
      ownerBaseAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { base: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: baseMint,
        collateralVault: baseCollateralVault,
        ownerAssetAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .borrow({
          borrowAsset: { base: {} },
          borrowAmount: new BN(5),
          minDebtAmountOut: new BN(5),
          minHealthBps: new BN(11_000),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          ownerDebtAccount: ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, ownerBaseAccount)).amount).to.equal(
      BigInt(240)
    );
    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, baseCollateralVault)).amount).to.equal(
      BigInt(60)
    );
  });

  it("blocks market repays when spot diverges from cached EMA", async () => {
    const {
      baseMint,
      quoteMint,
      baseClaimTokenMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseClaimAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    await addLiquiditySide(
      { market, eventAuthority },
      0,
      baseMint,
      baseClaimTokenMint,
      baseReserveVault,
      ownerBaseAccount,
      ownerBaseClaimAccount,
      260,
      208,
      52
    );

    await program.methods
      .updateConfig({ config: marketConfig() })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await expectRejects(() =>
      program.methods
        .repay({
          repayAsset: { base: {} },
          repayAmount: new BN(5),
        })
        .accounts({
          market,
          owner: payer.publicKey,
          debtAssetMint: baseMint,
          reserveVault: baseReserveVault,
          ownerDebtAccount: ownerBaseAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([payer])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
        .rpc()
    );
  });

  async function fundExhaustibleLiquidationMarket(insuranceAmount = 0) {
    const fixture = await fundRoundedBorrowMarket();
    const {
      baseMint,
      quoteMint,
      quoteClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerQuoteClaimAccount,
      eventAuthority,
    } = fixture;
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(6),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
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
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await mintTo(
      connection as any,
      payer,
      quoteMint,
      ownerQuoteAccount,
      payer,
      300
    );
    await addLiquiditySide(
      { market, eventAuthority },
      1,
      quoteMint,
      quoteClaimTokenMint,
      quoteReserveVault,
      ownerQuoteAccount,
      ownerQuoteClaimAccount,
      300,
      240,
      60
    );

    if (insuranceAmount > 0) {
      await program.methods
        .depositInsurance({
          marketAsset: { base: {} },
          depositAmount: new BN(insuranceAmount),
        })
        .accounts({
          market,
          sponsor: payer.publicKey,
          assetMint: baseMint,
          insuranceVault: baseInsuranceVault,
          sponsorAssetAccount: ownerBaseAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
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
      baseMint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      quoteMint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      baseMint,
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
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      baseMint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      quoteMint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      baseMint,
      liquidatorDebtAccount,
      payer,
      10
    );

    await expectRejects(() =>
      program.methods
        .liquidate({
          debtAsset: { base: {} },
          repayAmount: new BN(5),
          minCollateralOut: new BN(1),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          collateralVault: quoteCollateralVault,
          insuranceVault: baseInsuranceVault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(307)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
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
    trackInstruction("liquidate", "liquidates fixed market debt");

    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const config = marketConfig();
    config.recognizedCollateralCapBps = 20_000;
    config.marketHealthMinBps = 20_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      baseMint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      quoteMint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      baseMint,
      liquidatorDebtAccount,
      payer,
      10
    );

    const liquidationSignature = await program.methods
      .liquidate({
        debtAsset: { base: {} },
        repayAmount: new BN(5),
        minCollateralOut: new BN(1),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        collateralVault: quoteCollateralVault,
        insuranceVault: baseInsuranceVault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    const liquidationEvents = decodeCpiEvents(svm, liquidationSignature);
    const liquidationEventNames = liquidationEvents.map((event) => event.name);
    expect(liquidationEventNames).to.include("PositionLiquidated");
    expect(liquidationEventNames).to.include("MarketHealthUpdated");
    const healthEvent = liquidationEvents.find((event) => event.name === "MarketHealthUpdated");
    expect(healthEvent.data.market.toString()).to.equal(market.toString());
    expect(healthEvent.data.effective_base_debt_nad.toString()).to.equal("0");

    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
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
      quoteCollateralVault
    )).amount;
    expect(liquidatorCollateralBalance > BigInt(0)).to.equal(true);
    expect(collateralVaultBalance < BigInt(60)).to.equal(true);
  });

  it("uses insurance when liquidation exhausts collateral", async () => {
    const {
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      liquidator,
      liquidatorDebtAccount,
      liquidatorCollateralAccount,
      marginPosition,
      eventAuthority,
    } = await fundExhaustibleLiquidationMarket(1);

    await expectRejects(() =>
      program.methods
        .liquidate({
          debtAsset: { base: {} },
          repayAmount: new BN(4),
          minCollateralOut: new BN(6),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          collateralVault: quoteCollateralVault,
          insuranceVault: baseInsuranceVault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    await program.methods
      .liquidate({
        debtAsset: { base: {} },
        repayAmount: new BN(4),
        minCollateralOut: new BN(6),
        maxInsuranceDraw: new BN(1),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        collateralVault: quoteCollateralVault,
        insuranceVault: baseInsuranceVault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(312)
    );
    expect((await getAccount(connection as any, baseInsuranceVault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
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
      baseMint,
      quoteMint,
      market,
      baseReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      liquidator,
      liquidatorDebtAccount,
      liquidatorCollateralAccount,
      marginPosition,
      eventAuthority,
    } = await fundExhaustibleLiquidationMarket(0);

    await expectRejects(() =>
      program.methods
        .liquidate({
          debtAsset: { base: {} },
          repayAmount: new BN(4),
          minCollateralOut: new BN(6),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          collateralVault: quoteCollateralVault,
          insuranceVault: baseInsuranceVault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
        })
        .signers([liquidator])
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
        .rpc()
    );

    await program.methods
      .liquidate({
        debtAsset: { base: {} },
        repayAmount: new BN(4),
        minCollateralOut: new BN(6),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(1),
      })
      .accounts({
        market,
        liquidator: liquidator.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        collateralVault: quoteCollateralVault,
        insuranceVault: baseInsuranceVault,
        liquidatorDebtAccount,
        liquidatorCollateralAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([liquidator])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();

    expect((await getAccount(connection as any, baseReserveVault)).amount).to.equal(
      BigInt(311)
    );
    expect((await getAccount(connection as any, baseInsuranceVault)).amount).to.equal(
      BigInt(0)
    );
    expect((await getAccount(connection as any, quoteCollateralVault)).amount).to.equal(
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
      baseMint,
      quoteMint,
      quoteClaimTokenMint,
      market,
      baseReserveVault,
      quoteReserveVault,
      quoteCollateralVault,
      baseInsuranceVault,
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerQuoteClaimAccount,
      eventAuthority,
    } = await fundRoundedBorrowMarket();
    const marginPosition = deriveAddress(
      Buffer.from("margin"),
      market.toBuffer(),
      payer.publicKey.toBuffer()
    );

    await program.methods
      .depositCollateral({
        marketAsset: { quote: {} },
        depositAmount: new BN(60),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        assetMint: quoteMint,
        collateralVault: quoteCollateralVault,
        ownerAssetAccount: ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await program.methods
      .borrow({
        borrowAsset: { base: {} },
        borrowAmount: new BN(5),
        minDebtAmountOut: new BN(5),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market,
        owner: payer.publicKey,
        debtAssetMint: baseMint,
        collateralAssetMint: quoteMint,
        reserveVault: baseReserveVault,
        ownerDebtAccount: ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .rpc();

    const config = syntheticLiquidityRiskConfig();
    config.recognizedCollateralCapBps = 20_000;
    config.marketHealthMinBps = 20_000;
    await program.methods
      .updateConfig({ config })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    await addLiquiditySide(
      { market, eventAuthority },
      1,
      quoteMint,
      quoteClaimTokenMint,
      quoteReserveVault,
      ownerQuoteAccount,
      ownerQuoteClaimAccount,
      200,
      160,
      40
    );

    const strictRiskConfig = marketConfig();
    strictRiskConfig.recognizedCollateralCapBps = config.recognizedCollateralCapBps;
    strictRiskConfig.marketHealthMinBps = config.marketHealthMinBps;
    await program.methods
      .updateConfig({ config: strictRiskConfig })
      .accounts({
        market,
        operator: payer.publicKey,
        eventAuthority,
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();

    const liquidator = Keypair.generate();
    await connection.requestAirdrop(liquidator.publicKey, LAMPORTS_PER_SOL);
    const liquidatorDebtAccount = await createAccount(
      connection as any,
      payer,
      baseMint,
      liquidator.publicKey
    );
    const liquidatorCollateralAccount = await createAccount(
      connection as any,
      payer,
      quoteMint,
      liquidator.publicKey
    );
    await mintTo(
      connection as any,
      payer,
      baseMint,
      liquidatorDebtAccount,
      payer,
      10
    );

    await expectRejects(() =>
      program.methods
        .liquidate({
          debtAsset: { base: {} },
          repayAmount: new BN(5),
          minCollateralOut: new BN(1),
          maxInsuranceDraw: new BN(0),
          maxSocializedLoss: new BN(0),
        })
        .accounts({
          market,
          liquidator: liquidator.publicKey,
          debtAssetMint: baseMint,
          collateralAssetMint: quoteMint,
          reserveVault: baseReserveVault,
          collateralVault: quoteCollateralVault,
          insuranceVault: baseInsuranceVault,
          liquidatorDebtAccount,
          liquidatorCollateralAccount,
          marginPosition,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          eventAuthority,
          program: OMNIPAIR_V2_PROGRAM_ID,
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
