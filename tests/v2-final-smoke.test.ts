import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import anchor from "@coral-xyz/anchor";
import {
  ACCOUNT_SIZE,
  createAccount,
  createInitializeAccount3Instruction,
  createInitializeMintInstruction,
  createMint,
  createTransferCheckedWithTransferHookInstruction,
  ExtensionType,
  getAccount,
  getMintLen,
  mintTo,
  NATIVE_MINT,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createInitializeTransferHookInstruction,
} from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { expect } from "chai";
import { ComputeBudget, LiteSVM } from "litesvm";
import {
  buildYieldTransferHookAccountMetas,
  buildYieldTransferHookYieldValidationAccountData,
  deriveFutarchyAuthorityV2Address,
  deriveHlpYlpVaultAddress,
  deriveInsuranceAddress,
  deriveMarketAddress,
  deriveMarketCollateralVaultAddress,
  deriveMarketFeeVaultAddress,
  deriveMarketInterestVaultAddress,
  deriveMarketReserveVaultAddress,
  deriveMarginPositionAddress,
  deriveYieldAccountAddress,
  deriveYieldTransferHookValidationAddress,
} from "../packages/program-interface/src/constants.js";
import { LiteSVMConnection } from "./utils/litesvm-connection.js";
import { getCoverageReport, trackV2Instruction } from "./utils/instruction-coverage.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const { AnchorProvider, BN, Program, Wallet } = anchor;
const OMNIPAIR_V2_PROGRAM_ID = new PublicKey("358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv");
const idl = JSON.parse(
  fs.readFileSync(path.join(__dirname, "../target/idl/omnipair_v2.json"), "utf-8")
);
const accountCoder = new anchor.BorshAccountsCoder(idl);
const REDUCE_ONLY_EMERGENCY_AUTHORITY = new PublicKey(
  "3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV"
);
const BPF_LOADER_UPGRADEABLE_PROGRAM_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111"
);

function marketConfig() {
  return {
    swapFeeBps: 30,
    operatorFeeBps: 0,
    protocolFeeBps: 0,
    targetHlpLeverageBps: 20_000,
    settlementDivergenceBps: 500,
    emergencyExitHaircutBps: 250,
    emaHalfLifeMs: new BN(60_000),
    directionalEmaHalfLifeMs: new BN(60_000),
    kEmaHalfLifeMs: new BN(60_000),
    maxDailyBorrowBps: 2_000,
    maxDailyWithdrawBps: 2_000,
    spotEmaDivergenceBps: 1_000,
    kEmaDrawdownBps: 1_000,
    recognizedCollateralCapBps: 15_000,
    marketHealthMinBps: 11_000,
    hedgedLpEnabled: true,
    startTime: new BN(0),
  };
}

describe("Omnipair V2 final model smoke", () => {
  let svm: LiteSVM;
  let connection: LiteSVMConnection;
  let payer: Keypair;
  let program: any;
  let teamTreasury: PublicKey;
  let teamTreasuryWsolAccount: PublicKey;
  let futarchyAuthority: PublicKey;

  before(async () => {
    const computeBudget = new ComputeBudget();
    computeBudget.computeUnitLimit = 600_000n;
    svm = new LiteSVM().withComputeBudget(computeBudget);
    svm.warpToSlot(1n);
    const programPath = path.join(__dirname, "../target/deploy/omnipair_v2.so");
    if (!fs.existsSync(programPath)) {
      throw new Error(`Program file not found at ${programPath}`);
    }
    svm.addProgramFromFile(OMNIPAIR_V2_PROGRAM_ID, programPath);
    connection = new LiteSVMConnection(svm);

    payer = Keypair.generate();
    await connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL);
    const provider = new AnchorProvider(connection as any, new Wallet(payer) as any, {});
    program = new Program({ ...idl, accounts: [] } as any, provider as any);

    teamTreasury = Keypair.generate().publicKey;
    const teamTreasuryWsol = Keypair.generate();
    teamTreasuryWsolAccount = teamTreasuryWsol.publicKey;
    await connection.sendTransaction(
      new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: payer.publicKey,
          newAccountPubkey: teamTreasuryWsolAccount,
          lamports: await connection.getMinimumBalanceForRentExemption(ACCOUNT_SIZE),
          space: ACCOUNT_SIZE,
          programId: TOKEN_PROGRAM_ID,
        }),
        createInitializeAccount3Instruction(
          teamTreasuryWsolAccount,
          NATIVE_MINT,
          teamTreasury,
          TOKEN_PROGRAM_ID
        )
      ),
      [payer, teamTreasuryWsol]
    );

    await seedFutarchyAuthority();
  });

  after(() => {
    getCoverageReport();
  });

  beforeEach(async () => {
    await resetFutarchyDefaults();
  });

  async function seedFutarchyAuthority() {
    const [authority, bump] = deriveFutarchyAuthorityV2Address();
    futarchyAuthority = authority;
    const auctionRecipients = {
      treasury: payer.publicKey,
      staking_vault: payer.publicKey,
      treasury_bps: 10_000,
      staking_vault_bps: 0,
    };
    const auctionParams = {
      start_multiplier_bps: 12_000,
      floor_multiplier_bps: 8_000,
      duration_slots: new BN(216_000),
      max_reference_age_slots: new BN(21_600),
    };
    const auctionConfig = {
      accepted_mint: NATIVE_MINT,
      recipients: auctionRecipients,
      params: auctionParams,
      last_settlement_slot: new BN(0),
      last_settlement_price_nad: new BN(0),
    };
    const data = await accountCoder.encode("FutarchyAuthority", {
      version: 1,
      authority: payer.publicKey,
      recipients: {
        futarchy_treasury: payer.publicKey,
        buybacks_vault: payer.publicKey,
        team_treasury: teamTreasury,
      },
      revenue_share: {
        swap_bps: 0,
        interest_bps: 0,
      },
      revenue_distribution: {
        futarchy_treasury_bps: 0,
        buybacks_vault_bps: 0,
        team_treasury_bps: 10_000,
      },
      protocol_auction_split: {
        fee_auction_bps: 10_000,
        buyback_auction_bps: 0,
      },
      fee_auction: auctionConfig,
      buyback_auction: auctionConfig,
      global_reduce_only: false,
      bump,
    });
    svm.setAccount(futarchyAuthority, {
      lamports: Number(svm.minimumBalanceForRentExemption(BigInt(data.length))),
      data: new Uint8Array(data),
      owner: OMNIPAIR_V2_PROGRAM_ID,
      executable: false,
      rentEpoch: 0,
    });
  }

  async function resetFutarchyDefaults() {
    await seedFutarchyAuthority();
  }

  async function seedYieldAccount(
    address: PublicKey,
    owner: PublicKey,
    market: PublicKey,
    assetMint: PublicKey,
    tokenKind: "ylp" | "hlp",
    bump: number,
    recipient = owner
  ) {
    const data = await accountCoder.encode("YieldAccount", {
      owner,
      market,
      asset_mint: assetMint,
      token_kind: tokenKind === "ylp" ? 0 : 1,
      recipient,
      swap_fee_checkpoint_nad: new BN(0),
      interest_checkpoint_nad: new BN(0),
      accrued_swap_fee_amount: new BN(0),
      accrued_interest_amount: new BN(0),
      bump,
    });
    svm.setAccount(address, {
      lamports: Number(svm.minimumBalanceForRentExemption(BigInt(data.length))),
      data: new Uint8Array(data),
      owner: OMNIPAIR_V2_PROGRAM_ID,
      executable: false,
      rentEpoch: 0,
    });
  }

  function seedTransferHookValidationAccount(
    lpMint: PublicKey,
    market: PublicKey,
    assetMint: PublicKey,
    tokenKind: "ylp" | "hlp"
  ) {
    const validationAccount = deriveYieldTransferHookValidationAddress(lpMint)[0];
    const data = buildYieldTransferHookYieldValidationAccountData({
      market,
      assetMint,
      tokenKind,
    });
    svm.setAccount(validationAccount, {
      lamports: Number(svm.minimumBalanceForRentExemption(BigInt(data.length))),
      data: new Uint8Array(data),
      owner: OMNIPAIR_V2_PROGRAM_ID,
      executable: false,
      rentEpoch: 0,
    });
    return validationAccount;
  }

  async function createHookedLpMint(authority: PublicKey, decimals = 6) {
    const mint = Keypair.generate();
    const mintLen = getMintLen([ExtensionType.TransferHook]);
    await connection.sendTransaction(
      new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: payer.publicKey,
          newAccountPubkey: mint.publicKey,
          lamports: await connection.getMinimumBalanceForRentExemption(mintLen),
          space: mintLen,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        createInitializeTransferHookInstruction(
          mint.publicKey,
          payer.publicKey,
          OMNIPAIR_V2_PROGRAM_ID,
          TOKEN_2022_PROGRAM_ID
        ),
        createInitializeMintInstruction(
          mint.publicKey,
          decimals,
          authority,
          null,
          TOKEN_2022_PROGRAM_ID
        )
      ),
      [payer, mint]
    );
    return mint.publicKey;
  }

  function eventAuthority() {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      OMNIPAIR_V2_PROGRAM_ID
    )[0];
  }

  async function sendTransactionWithUncheckedSigners(
    transaction: Transaction,
    signers: Keypair[],
    uncheckedSigners: PublicKey[]
  ) {
    const { blockhash } = await connection.getLatestBlockhash();
    transaction.recentBlockhash = blockhash;
    transaction.feePayer = payer.publicKey;
    transaction.sign(...signers);
    for (const signer of uncheckedSigners) {
      transaction.addSignature(signer, Buffer.alloc(64));
    }

    svm.withSigverify(false);
    try {
      const result = svm.sendTransaction(transaction as any);
      if (result && typeof (result as any).err === "function") {
        const err = (result as any).err();
        if (err) {
          const meta = (result as any).meta?.();
          const prettyLogs = meta?.prettyLogs?.();
          throw new Error(`Transaction failed: ${err.toString?.() ?? err}\n${prettyLogs ?? ""}`);
        }
      }
      if (result && "err" in result && (result as any).err) {
        throw new Error(`Transaction failed: ${JSON.stringify((result as any).err)}`);
      }
    } finally {
      svm.withSigverify(true);
    }
  }

  function upgradeableProgramData(authority: PublicKey) {
    const data = Buffer.alloc(45);
    data.writeUInt32LE(3, 0);
    data.writeBigUInt64LE(0n, 4);
    data[12] = 1;
    authority.toBuffer().copy(data, 13);
    return data;
  }

  async function createIsolatedProgram() {
    const isolatedSvm = new LiteSVM().withComputeBudget(new ComputeBudget());
    const programPath = path.join(__dirname, "../target/deploy/omnipair_v2.so");
    isolatedSvm.addProgramFromFile(OMNIPAIR_V2_PROGRAM_ID, programPath);
    const isolatedConnection = new LiteSVMConnection(isolatedSvm);
    const isolatedPayer = Keypair.generate();
    await isolatedConnection.requestAirdrop(isolatedPayer.publicKey, 10 * LAMPORTS_PER_SOL);
    const isolatedProvider = new AnchorProvider(
      isolatedConnection as any,
      new Wallet(isolatedPayer) as any,
      {}
    );
    const isolatedProgram = new Program({ ...idl, accounts: [] } as any, isolatedProvider as any);
    return {
      isolatedSvm,
      isolatedConnection,
      isolatedPayer,
      isolatedProgram,
    };
  }

  async function initializeFinalMarket(paramsSeed: number, config = marketConfig()) {
    const baseMint = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const quoteMint = await createMint(connection as any, payer, payer.publicKey, null, 6);
    const paramsHash = Buffer.alloc(32, paramsSeed);
    const [market] = deriveMarketAddress(baseMint, quoteMint, paramsHash);
    const baseYlpMint = await createHookedLpMint(market, 6);
    const quoteYlpMint = await createHookedLpMint(market, 6);
    const baseHlpMint = await createHookedLpMint(market, 6);
    const quoteHlpMint = await createHookedLpMint(market, 6);
    const baseHlpBaseYlpVault = deriveHlpYlpVaultAddress(market, "base", baseYlpMint)[0];
    const baseHlpQuoteYlpVault = deriveHlpYlpVaultAddress(market, "base", quoteYlpMint)[0];
    const quoteHlpBaseYlpVault = deriveHlpYlpVaultAddress(market, "quote", baseYlpMint)[0];
    const quoteHlpQuoteYlpVault = deriveHlpYlpVaultAddress(market, "quote", quoteYlpMint)[0];
    const baseReserveVault = deriveMarketReserveVaultAddress(market, baseMint)[0];
    const quoteReserveVault = deriveMarketReserveVaultAddress(market, quoteMint)[0];
    const baseCollateralVault = deriveMarketCollateralVaultAddress(market, baseMint)[0];
    const quoteCollateralVault = deriveMarketCollateralVaultAddress(market, quoteMint)[0];
    const baseInsuranceVault = deriveInsuranceAddress(market, baseMint)[0];
    const quoteInsuranceVault = deriveInsuranceAddress(market, quoteMint)[0];
    const baseFeeVault = deriveMarketFeeVaultAddress(market, baseMint)[0];
    const quoteFeeVault = deriveMarketFeeVaultAddress(market, quoteMint)[0];
    const baseInterestVault = deriveMarketInterestVaultAddress(market, baseMint)[0];
    const quoteInterestVault = deriveMarketInterestVaultAddress(market, quoteMint)[0];

    const tx = await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config,
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        baseMint,
        quoteMint,
        market,
        futarchyAuthority,
        baseYlpMint,
        quoteYlpMint,
        baseHlpMint,
        quoteHlpMint,
        baseReserveVault,
        quoteReserveVault,
        baseCollateralVault,
        quoteCollateralVault,
        baseInsuranceVault,
        quoteInsuranceVault,
        baseFeeVault,
        quoteFeeVault,
        baseInterestVault,
        quoteInterestVault,
        teamTreasury,
        teamTreasuryWsolAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);
    return {
      baseMint,
      quoteMint,
      paramsHash,
      market,
      baseYlpMint,
      quoteYlpMint,
      baseHlpMint,
      quoteHlpMint,
      baseHlpBaseYlpVault,
      baseHlpQuoteYlpVault,
      quoteHlpBaseYlpVault,
      quoteHlpQuoteYlpVault,
      baseReserveVault,
      quoteReserveVault,
      baseCollateralVault,
      quoteCollateralVault,
      baseInsuranceVault,
      quoteInsuranceVault,
      baseFeeVault,
      quoteFeeVault,
      baseInterestVault,
      quoteInterestVault,
    };
  }

  async function createOwnerAssetAccounts(fixture: Awaited<ReturnType<typeof initializeFinalMarket>>) {
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
    const ownerBaseYlpAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseYlpMint,
      payer.publicKey,
      Keypair.generate(),
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const ownerQuoteYlpAccount = await createAccount(
      connection as any,
      payer,
      fixture.quoteYlpMint,
      payer.publicKey,
      Keypair.generate(),
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    await mintTo(connection as any, payer, fixture.baseMint, ownerBaseAccount, payer, 1_000_000);
    await mintTo(connection as any, payer, fixture.quoteMint, ownerQuoteAccount, payer, 2_000_000);
    return {
      ownerBaseAccount,
      ownerQuoteAccount,
      ownerBaseYlpAccount,
      ownerQuoteYlpAccount,
    };
  }

  async function createRecipientAssetAccounts(
    fixture: Awaited<ReturnType<typeof initializeFinalMarket>>,
    owner: PublicKey
  ) {
    const baseAccount = await createAccount(connection as any, payer, fixture.baseMint, owner);
    const quoteAccount = await createAccount(connection as any, payer, fixture.quoteMint, owner);
    return { baseAccount, quoteAccount };
  }

  async function addBalancedLiquidity(paramsSeed: number, config = marketConfig()) {
    const fixture = await initializeFinalMarket(paramsSeed, config);
    const ownerAccounts = await createOwnerAssetAccounts(fixture);

    const tx = await program.methods
      .addLiquidity({
        baseDepositAmount: new BN(100_000),
        quoteDepositAmount: new BN(200_000),
        minBaseYlpAmount: new BN(100_000),
        minQuoteYlpAmount: new BN(200_000),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerBaseAccount: ownerAccounts.ownerBaseAccount,
        ownerQuoteAccount: ownerAccounts.ownerQuoteAccount,
        ownerBaseYlpAccount: ownerAccounts.ownerBaseYlpAccount,
        ownerQuoteYlpAccount: ownerAccounts.ownerQuoteYlpAccount,
        baseYieldAccount: deriveYieldAccountAddress(
          fixture.market,
          payer.publicKey,
          fixture.baseMint,
          "ylp"
        )[0],
        quoteYieldAccount: deriveYieldAccountAddress(
          fixture.market,
          payer.publicKey,
          fixture.quoteMint,
          "ylp"
        )[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);

    return {
      ...fixture,
      ...ownerAccounts,
    };
  }

  async function openBaseHedge(
    fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>,
    depositAmount = 10_000,
    existingOwnerBaseHlpAccount?: PublicKey
  ) {
    const ownerBaseHlpAccount =
      existingOwnerBaseHlpAccount ??
      (await createAccount(
        connection as any,
        payer,
        fixture.baseHlpMint,
        payer.publicKey,
        Keypair.generate(),
        undefined,
        TOKEN_2022_PROGRAM_ID
      ));
    const hlpBaseYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      "base",
      fixture.baseYlpMint
    )[0];
    const hlpQuoteYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      "base",
      fixture.quoteYlpMint
    )[0];
    const targetYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.baseMint,
      "hlp"
    )[0];

    const tx = await program.methods
      .openHedge({
        targetAsset: { base: {} },
        depositAmount: new BN(depositAmount),
        minHlpAmount: new BN(1),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        targetHlpMint: fixture.baseHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerTargetAccount: fixture.ownerBaseAccount,
        ownerHlpAccount: ownerBaseHlpAccount,
        hlpBaseYlpAccount,
        hlpQuoteYlpAccount,
        targetYieldAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);

    return {
      ownerBaseHlpAccount,
      hlpBaseYlpAccount,
      hlpQuoteYlpAccount,
      targetYieldAccount,
    };
  }

  async function openQuoteHedge(
    fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>,
    depositAmount = 20_000,
    existingOwnerQuoteHlpAccount?: PublicKey
  ) {
    const ownerQuoteHlpAccount =
      existingOwnerQuoteHlpAccount ??
      (await createAccount(
        connection as any,
        payer,
        fixture.quoteHlpMint,
        payer.publicKey,
        Keypair.generate(),
        undefined,
        TOKEN_2022_PROGRAM_ID
      ));
    const hlpBaseYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      "quote",
      fixture.baseYlpMint
    )[0];
    const hlpQuoteYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      "quote",
      fixture.quoteYlpMint
    )[0];
    const targetYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.quoteMint,
      "hlp"
    )[0];

    const tx = await program.methods
      .openHedge({
        targetAsset: { quote: {} },
        depositAmount: new BN(depositAmount),
        minHlpAmount: new BN(1),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        targetHlpMint: fixture.quoteHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerTargetAccount: fixture.ownerQuoteAccount,
        ownerHlpAccount: ownerQuoteHlpAccount,
        hlpBaseYlpAccount,
        hlpQuoteYlpAccount,
        targetYieldAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);

    return {
      ownerQuoteHlpAccount,
      hlpBaseYlpAccount,
      hlpQuoteYlpAccount,
      targetYieldAccount,
    };
  }

  function baseHlpRebalanceAccounts(fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>) {
    return [
      {
        pubkey: fixture.baseYlpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.quoteYlpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.baseHlpBaseYlpVault,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.baseHlpQuoteYlpVault,
        isWritable: true,
        isSigner: false,
      },
    ];
  }

  function quoteHlpRebalanceAccounts(fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>) {
    return [
      {
        pubkey: fixture.baseYlpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.quoteYlpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.quoteHlpBaseYlpVault,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.quoteHlpQuoteYlpVault,
        isWritable: true,
        isSigner: false,
      },
    ];
  }

  function allHlpRebalanceAccounts(fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>) {
    return [...baseHlpRebalanceAccounts(fixture), ...quoteHlpRebalanceAccounts(fixture)];
  }

  async function swapBaseForQuote(
    fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>,
    remainingAccounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[] = [],
    exactAssetIn = 1_000,
    minAssetOut = 1_900
  ) {
    let builder = program.methods
      .swap({
        assetIn: { base: {} },
        exactAssetIn: new BN(exactAssetIn),
        minAssetOut: new BN(minAssetOut),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        trader: payer.publicKey,
        assetInMint: fixture.baseMint,
        assetOutMint: fixture.quoteMint,
        reserveInVault: fixture.baseReserveVault,
        reserveOutVault: fixture.quoteReserveVault,
        feeInVault: fixture.baseFeeVault,
        traderAssetInAccount: fixture.ownerBaseAccount,
        traderAssetOutAccount: fixture.ownerQuoteAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      });
    if (remainingAccounts.length > 0) {
      builder = builder.remainingAccounts(remainingAccounts);
    }
    const tx = await builder.transaction();
    await connection.sendTransaction(tx, [payer]);
  }

  async function swapQuoteForBase(
    fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>,
    remainingAccounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[] = [],
    exactAssetIn = 2_000,
    minAssetOut = 900
  ) {
    let builder = program.methods
      .swap({
        assetIn: { quote: {} },
        exactAssetIn: new BN(exactAssetIn),
        minAssetOut: new BN(minAssetOut),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        trader: payer.publicKey,
        assetInMint: fixture.quoteMint,
        assetOutMint: fixture.baseMint,
        reserveInVault: fixture.quoteReserveVault,
        reserveOutVault: fixture.baseReserveVault,
        feeInVault: fixture.quoteFeeVault,
        traderAssetInAccount: fixture.ownerQuoteAccount,
        traderAssetOutAccount: fixture.ownerBaseAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      });
    if (remainingAccounts.length > 0) {
      builder = builder.remainingAccounts(remainingAccounts);
    }
    const tx = await builder.transaction();
    await connection.sendTransaction(tx, [payer]);
  }

  it("initializes a final yLP/hLP market with hooked Token-2022 LP mints", async function () {
    const fixture = await initializeFinalMarket(42);
    trackV2Instruction("initialize", this.test?.title);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_mint.toString()).to.equal(fixture.baseMint.toString());
    expect(decoded.quote_mint.toString()).to.equal(fixture.quoteMint.toString());
    expect(decoded.base_side.ylp_mint.toString()).to.equal(fixture.baseYlpMint.toString());
    expect(decoded.quote_side.ylp_mint.toString()).to.equal(fixture.quoteYlpMint.toString());
    expect(decoded.base_side.hlp_mint.toString()).to.equal(fixture.baseHlpMint.toString());
    expect(decoded.quote_side.hlp_mint.toString()).to.equal(fixture.quoteHlpMint.toString());
    expect(decoded.base_hlp_vault.base_ylp_vault.toString()).to.equal(
      fixture.baseHlpBaseYlpVault.toString()
    );
    expect(decoded.base_hlp_vault.quote_ylp_vault.toString()).to.equal(
      fixture.baseHlpQuoteYlpVault.toString()
    );
    expect(decoded.quote_hlp_vault.base_ylp_vault.toString()).to.equal(
      fixture.quoteHlpBaseYlpVault.toString()
    );
    expect(decoded.quote_hlp_vault.quote_ylp_vault.toString()).to.equal(
      fixture.quoteHlpQuoteYlpVault.toString()
    );
  });

  it("initializes the V2 futarchy authority from upgradeable ProgramData", async function () {
    const { isolatedSvm, isolatedConnection, isolatedPayer, isolatedProgram } =
      await createIsolatedProgram();
    const [isolatedFutarchyAuthority] = deriveFutarchyAuthorityV2Address();
    const [programData] = PublicKey.findProgramAddressSync(
      [OMNIPAIR_V2_PROGRAM_ID.toBuffer()],
      BPF_LOADER_UPGRADEABLE_PROGRAM_ID
    );
    const programDataBytes = upgradeableProgramData(isolatedPayer.publicKey);
    isolatedSvm.setAccount(programData, {
      lamports: Number(isolatedSvm.minimumBalanceForRentExemption(BigInt(programDataBytes.length))),
      data: new Uint8Array(programDataBytes),
      owner: BPF_LOADER_UPGRADEABLE_PROGRAM_ID,
      executable: false,
      rentEpoch: 0,
    });

    const tx = await isolatedProgram.methods
      .initFutarchyAuthority({
        authority: isolatedPayer.publicKey,
        swapBps: 125,
        interestBps: 250,
        futarchyTreasury: isolatedPayer.publicKey,
        futarchyTreasuryBps: 5_000,
        buybacksVault: isolatedPayer.publicKey,
        buybacksVaultBps: 2_000,
        teamTreasury: isolatedPayer.publicKey,
        teamTreasuryBps: 3_000,
      })
      .accounts({
        deployer: isolatedPayer.publicKey,
        futarchyAuthority: isolatedFutarchyAuthority,
        programData,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await isolatedConnection.sendTransaction(tx, [isolatedPayer]);
    trackV2Instruction("initFutarchyAuthority", this.test?.title);

    const authorityAccount = isolatedSvm.getAccount(isolatedFutarchyAuthority);
    expect(authorityAccount).to.not.equal(null);
    const authority = accountCoder.decode(
      "FutarchyAuthority",
      Buffer.from(authorityAccount!.data)
    ) as any;
    expect(authority.authority.toString()).to.equal(isolatedPayer.publicKey.toString());
    expect(authority.revenue_share.swap_bps).to.equal(125);
    expect(authority.revenue_share.interest_bps).to.equal(250);
    expect(authority.revenue_distribution.futarchy_treasury_bps).to.equal(5_000);
    expect(authority.revenue_distribution.buybacks_vault_bps).to.equal(2_000);
    expect(authority.revenue_distribution.team_treasury_bps).to.equal(3_000);
  });

  it("adds balanced liquidity and mints floating yLP shares", async function () {
    const fixture = await addBalancedLiquidity(43);
    trackV2Instruction("addLiquidity", this.test?.title);

    const baseYlpAccount = await getAccount(
      connection as any,
      fixture.ownerBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpAccount = await getAccount(
      connection as any,
      fixture.ownerQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseYlpAccount.amount).to.equal(100_000n);
    expect(quoteYlpAccount.amount).to.equal(200_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(200_000);
  });

  it("opens base hLP by borrowing quote and locking both yLP sides", async function () {
    const fixture = await addBalancedLiquidity(44);
    const ownerBaseBefore = await getAccount(connection as any, fixture.ownerBaseAccount);
    const hedge = await openBaseHedge(fixture);
    trackV2Instruction("openHedge", this.test?.title);

    const ownerBaseAfter = await getAccount(connection as any, fixture.ownerBaseAccount);
    expect(ownerBaseAfter.amount).to.equal(ownerBaseBefore.amount - 10_000n);

    const ownerHlp = await getAccount(
      connection as any,
      hedge.ownerBaseHlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultBaseYlp = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultQuoteYlp = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(10_000n);
    expect(vaultBaseYlp.amount).to.equal(10_000n);
    expect(vaultQuoteYlp.amount).to.equal(20_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(110_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(220_000);
    expect(decoded.base_hlp_vault.ylp_base_shares.toNumber()).to.equal(10_000);
    expect(decoded.base_hlp_vault.ylp_quote_shares.toNumber()).to.equal(20_000);
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(10_000);
    expect(decoded.base_hlp_vault.debt_shares.toNumber()).to.be.greaterThan(0);
  });

  it("aggregates repeated base hLP opens into canonical vault yLP accounts", async function () {
    const fixture = await addBalancedLiquidity(50);
    const first = await openBaseHedge(fixture, 5_000);
    await openBaseHedge(fixture, 6_000, first.ownerBaseHlpAccount);

    const ownerHlp = await getAccount(
      connection as any,
      first.ownerBaseHlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultBaseYlp = await getAccount(
      connection as any,
      first.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultQuoteYlp = await getAccount(
      connection as any,
      first.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(11_000n);
    expect(vaultBaseYlp.amount).to.equal(11_000n);
    expect(vaultQuoteYlp.amount).to.equal(22_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_hlp_vault.ylp_base_shares.toNumber()).to.equal(11_000);
    expect(decoded.base_hlp_vault.ylp_quote_shares.toNumber()).to.equal(22_000);
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(11_000);
  });

  it("closes base hLP by burning vault yLP, repaying quote debt, and returning base", async function () {
    const fixture = await addBalancedLiquidity(45);
    const ownerBaseBeforeOpen = await getAccount(connection as any, fixture.ownerBaseAccount);
    const hedge = await openBaseHedge(fixture);

    const tx = await program.methods
      .closeHedge({
        targetAsset: { base: {} },
        hlpAmount: new BN(10_000),
        minTargetAmountOut: new BN(9_999),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        targetHlpMint: fixture.baseHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        borrowedInterestVault: fixture.quoteInterestVault,
        ownerTargetAccount: fixture.ownerBaseAccount,
        ownerHlpAccount: hedge.ownerBaseHlpAccount,
        hlpBaseYlpAccount: hedge.hlpBaseYlpAccount,
        hlpQuoteYlpAccount: hedge.hlpQuoteYlpAccount,
        targetYieldAccount: hedge.targetYieldAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);
    trackV2Instruction("closeHedge", this.test?.title);

    const ownerBaseAfterClose = await getAccount(connection as any, fixture.ownerBaseAccount);
    expect(ownerBaseAfterClose.amount).to.equal(ownerBaseBeforeOpen.amount);

    const ownerHlp = await getAccount(
      connection as any,
      hedge.ownerBaseHlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultBaseYlp = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultQuoteYlp = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(0n);
    expect(vaultBaseYlp.amount).to.equal(0n);
    expect(vaultQuoteYlp.amount).to.equal(0n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(200_000);
    expect(decoded.base_hlp_vault.ylp_base_shares.toNumber()).to.equal(0);
    expect(decoded.base_hlp_vault.ylp_quote_shares.toNumber()).to.equal(0);
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(0);
    expect(decoded.base_hlp_vault.debt_shares.toNumber()).to.equal(0);
  });

  it("opens and closes quote hLP by borrowing base and returning quote", async function () {
    const fixture = await addBalancedLiquidity(54);
    const ownerQuoteBeforeOpen = await getAccount(connection as any, fixture.ownerQuoteAccount);
    const hedge = await openQuoteHedge(fixture);
    trackV2Instruction("openHedge", this.test?.title);

    const ownerQuoteAfterOpen = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerQuoteAfterOpen.amount).to.equal(ownerQuoteBeforeOpen.amount - 20_000n);

    const ownerHlp = await getAccount(
      connection as any,
      hedge.ownerQuoteHlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultBaseYlp = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const vaultQuoteYlp = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(20_000n);
    expect(vaultBaseYlp.amount).to.equal(10_000n);
    expect(vaultQuoteYlp.amount).to.equal(20_000n);

    let account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    let decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.quote_hlp_vault.ylp_base_shares.toNumber()).to.equal(10_000);
    expect(decoded.quote_hlp_vault.ylp_quote_shares.toNumber()).to.equal(20_000);
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(20_000);
    expect(decoded.quote_hlp_vault.debt_shares.toNumber()).to.be.greaterThan(0);

    const tx = await program.methods
      .closeHedge({
        targetAsset: { quote: {} },
        hlpAmount: new BN(20_000),
        minTargetAmountOut: new BN(19_999),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        targetHlpMint: fixture.quoteHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        borrowedInterestVault: fixture.baseInterestVault,
        ownerTargetAccount: fixture.ownerQuoteAccount,
        ownerHlpAccount: hedge.ownerQuoteHlpAccount,
        hlpBaseYlpAccount: hedge.hlpBaseYlpAccount,
        hlpQuoteYlpAccount: hedge.hlpQuoteYlpAccount,
        targetYieldAccount: hedge.targetYieldAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);
    trackV2Instruction("closeHedge", this.test?.title);

    const ownerQuoteAfterClose = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerQuoteAfterClose.amount).to.equal(ownerQuoteBeforeOpen.amount);

    account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.quote_hlp_vault.ylp_base_shares.toNumber()).to.equal(0);
    expect(decoded.quote_hlp_vault.ylp_quote_shares.toNumber()).to.equal(0);
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(0);
    expect(decoded.quote_hlp_vault.debt_shares.toNumber()).to.equal(0);
  });

  it("removes matched yLP liquidity and returns pro-rata reserves", async function () {
    const fixture = await addBalancedLiquidity(46);
    const ownerBaseBefore = await getAccount(connection as any, fixture.ownerBaseAccount);
    const ownerQuoteBefore = await getAccount(connection as any, fixture.ownerQuoteAccount);

    const tx = await program.methods
      .removeLiquidity({
        baseYlpAmount: new BN(1_000),
        quoteYlpAmount: new BN(2_000),
        minBaseAmountOut: new BN(1_000),
        minQuoteAmountOut: new BN(2_000),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        baseYlpMint: fixture.baseYlpMint,
        quoteYlpMint: fixture.quoteYlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerBaseAccount: fixture.ownerBaseAccount,
        ownerQuoteAccount: fixture.ownerQuoteAccount,
        ownerBaseYlpAccount: fixture.ownerBaseYlpAccount,
        ownerQuoteYlpAccount: fixture.ownerQuoteYlpAccount,
        baseYieldAccount: deriveYieldAccountAddress(
          fixture.market,
          payer.publicKey,
          fixture.baseMint,
          "ylp"
        )[0],
        quoteYieldAccount: deriveYieldAccountAddress(
          fixture.market,
          payer.publicKey,
          fixture.quoteMint,
          "ylp"
        )[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);
    trackV2Instruction("removeLiquidity", this.test?.title);

    const ownerBaseAfter = await getAccount(connection as any, fixture.ownerBaseAccount);
    const ownerQuoteAfter = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerBaseAfter.amount).to.equal(ownerBaseBefore.amount + 1_000n);
    expect(ownerQuoteAfter.amount).to.equal(ownerQuoteBefore.amount + 2_000n);

    const baseYlpAccount = await getAccount(
      connection as any,
      fixture.ownerBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpAccount = await getAccount(
      connection as any,
      fixture.ownerQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseYlpAccount.amount).to.equal(99_000n);
    expect(quoteYlpAccount.amount).to.equal(198_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(99_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(198_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(99_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(198_000);
  });

  it("swaps through the V2 market and routes non-compounding swap fees", async function () {
    const fixture = await addBalancedLiquidity(47);
    const ownerQuoteBefore = await getAccount(connection as any, fixture.ownerQuoteAccount);

    await swapBaseForQuote(fixture);
    trackV2Instruction("swap", this.test?.title);

    const ownerQuoteAfter = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerQuoteAfter.amount).to.equal(ownerQuoteBefore.amount + 1_974n);

    const baseFeeVault = await getAccount(connection as any, fixture.baseFeeVault);
    expect(baseFeeVault.amount).to.equal(3n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_997);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(198_026);
    expect(decoded.base_side.fees.swap_fee_liability.toNumber()).to.equal(3);
  });

  it("updates V2 futarchy revenue, recipients, authority, and market config", async function () {
    const fixture = await initializeFinalMarket(52);
    const futarchyTreasury = Keypair.generate().publicKey;
    const buybacksVault = Keypair.generate().publicKey;
    const replacementTeamTreasury = Keypair.generate().publicKey;

    const updateRevenueTx = await program.methods
      .updateProtocolRevenue({
        swapBps: 10_000,
        interestBps: 250,
        revenueDistribution: {
          futarchyTreasuryBps: 0,
          buybacksVaultBps: 0,
          teamTreasuryBps: 10_000,
        },
        protocolAuctionSplit: null,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateRevenueTx, [payer]);
    trackV2Instruction("updateProtocolRevenue", this.test?.title);

    const updateRecipientsTx = await program.methods
      .updateRevenueRecipients({
        futarchyTreasury,
        buybacksVault,
        teamTreasury: replacementTeamTreasury,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateRecipientsTx, [payer]);
    trackV2Instruction("updateRevenueRecipients", this.test?.title);

    const updateAuthorityTx = await program.methods
      .updateFutarchyAuthority({
        newAuthority: payer.publicKey,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateAuthorityTx, [payer]);
    trackV2Instruction("updateFutarchyAuthority", this.test?.title);

    const updatedConfig = marketConfig();
    updatedConfig.swapFeeBps = 40;
    const updateConfigTx = await program.methods
      .updateConfig({
        config: updatedConfig,
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        authoritySigner: payer.publicKey,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(updateConfigTx, [payer]);
    trackV2Instruction("updateConfig", this.test?.title);

    const authorityAccount = svm.getAccount(futarchyAuthority);
    expect(authorityAccount).to.not.equal(null);
    const authority = accountCoder.decode(
      "FutarchyAuthority",
      Buffer.from(authorityAccount!.data)
    ) as any;
    expect(authority.revenue_share.swap_bps).to.equal(10_000);
    expect(authority.revenue_share.interest_bps).to.equal(250);
    expect(authority.recipients.futarchy_treasury.toString()).to.equal(
      futarchyTreasury.toString()
    );
    expect(authority.recipients.buybacks_vault.toString()).to.equal(buybacksVault.toString());
    expect(authority.recipients.team_treasury.toString()).to.equal(
      replacementTeamTreasury.toString()
    );

    const marketAccount = svm.getAccount(fixture.market);
    expect(marketAccount).to.not.equal(null);
    const decodedMarket = accountCoder.decode("Market", Buffer.from(marketAccount!.data)) as any;
    expect(decodedMarket.config.swap_fee_bps).to.equal(30);
    expect(decodedMarket.pending_config.active).to.equal(true);
    expect(decodedMarket.pending_config.config.swap_fee_bps).to.equal(40);

    await resetFutarchyDefaults();
  });

  it("toggles global and market reduce-only through the emergency signer", async function () {
    const fixture = await initializeFinalMarket(57);

    const globalTx = await program.methods
      .setGlobalReduceOnly({
        reduceOnly: true,
      })
      .accounts({
        authoritySigner: REDUCE_ONLY_EMERGENCY_AUTHORITY,
        futarchyAuthority,
      })
      .transaction();
    await sendTransactionWithUncheckedSigners(globalTx, [payer], [REDUCE_ONLY_EMERGENCY_AUTHORITY]);
    trackV2Instruction("setGlobalReduceOnly", this.test?.title);

    let authorityAccount = svm.getAccount(futarchyAuthority);
    expect(authorityAccount).to.not.equal(null);
    let authority = accountCoder.decode(
      "FutarchyAuthority",
      Buffer.from(authorityAccount!.data)
    ) as any;
    expect(authority.global_reduce_only).to.equal(true);

    const marketTx = await program.methods
      .setReduceOnly({
        reduceOnly: true,
      })
      .accounts({
        market: fixture.market,
        authoritySigner: REDUCE_ONLY_EMERGENCY_AUTHORITY,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await sendTransactionWithUncheckedSigners(marketTx, [payer], [REDUCE_ONLY_EMERGENCY_AUTHORITY]);
    trackV2Instruction("setReduceOnly", this.test?.title);

    const marketAccount = svm.getAccount(fixture.market);
    expect(marketAccount).to.not.equal(null);
    const decodedMarket = accountCoder.decode("Market", Buffer.from(marketAccount!.data)) as any;
    expect(decodedMarket.reduce_only).to.equal(true);

    await resetFutarchyDefaults();
    authorityAccount = svm.getAccount(futarchyAuthority);
    expect(authorityAccount).to.not.equal(null);
    authority = accountCoder.decode("FutarchyAuthority", Buffer.from(authorityAccount!.data)) as any;
    expect(authority.global_reduce_only).to.equal(false);
  });

  it("settles protocol swap fees through the fee auction lane", async function () {
    const fixture = await addBalancedLiquidity(53);
    const treasury = Keypair.generate().publicKey;
    const stakingVault = Keypair.generate().publicKey;
    const treasuryAccounts = await createRecipientAssetAccounts(fixture, treasury);
    const stakingAccounts = await createRecipientAssetAccounts(fixture, stakingVault);

    const updateAuctionConfigTx = await program.methods
      .updateProtocolAuctionConfig({
        lane: { fee: {} },
        acceptedMint: fixture.quoteMint,
        params: null,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateAuctionConfigTx, [payer]);
    trackV2Instruction("updateProtocolAuctionConfig", this.test?.title);

    const updateAuctionRecipientsTx = await program.methods
      .updateProtocolAuctionRecipients({
        lane: { fee: {} },
        treasury,
        stakingVault,
        treasuryBps: 10_000,
        stakingVaultBps: 0,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateAuctionRecipientsTx, [payer]);
    trackV2Instruction("updateProtocolAuctionRecipients", this.test?.title);

    const updateRevenueTx = await program.methods
      .updateProtocolRevenue({
        swapBps: 10_000,
        interestBps: 0,
        revenueDistribution: null,
        protocolAuctionSplit: null,
      })
      .accounts({
        authoritySigner: payer.publicKey,
        futarchyAuthority,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(updateRevenueTx, [payer]);
    trackV2Instruction("updateProtocolRevenue", this.test?.title);

    await swapBaseForQuote(fixture);

    const settleTx = await program.methods
      .settleProtocolAuction({
        lane: { fee: {} },
        side: { base: {} },
        soldAmount: new BN(3),
        maxPaymentAmount: new BN(1_000),
      })
      .accounts({
        bidder: payer.publicKey,
        market: fixture.market,
        futarchyAuthority,
        soldMint: fixture.baseMint,
        acceptedMint: fixture.quoteMint,
        soldFeeVault: fixture.baseFeeVault,
        bidderPaymentAccount: fixture.ownerQuoteAccount,
        bidderReceiveAccount: fixture.ownerBaseAccount,
        treasuryPaymentAccount: treasuryAccounts.quoteAccount,
        stakingVaultPaymentAccount: stakingAccounts.quoteAccount,
        referenceMarket: fixture.market,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(settleTx, [payer]);
    trackV2Instruction("settleProtocolAuction", this.test?.title);

    const treasuryQuoteBalance = await getAccount(connection as any, treasuryAccounts.quoteAccount);
    expect(treasuryQuoteBalance.amount > 0n).to.equal(true);
    const baseFeeVault = await getAccount(connection as any, fixture.baseFeeVault);
    expect(baseFeeVault.amount).to.equal(0n);

    const marketAccount = svm.getAccount(fixture.market);
    expect(marketAccount).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(marketAccount!.data)) as any;
    expect(decoded.base_side.fees.protocol_fee_liability.toNumber()).to.equal(0);
    expect(decoded.base_side.fees.swap_fee_vault_balance.toNumber()).to.equal(0);

    await resetFutarchyDefaults();
  });

  it("checkpoints active hLP vaults during swaps with canonical vault accounts", async function () {
    const fixture = await addBalancedLiquidity(51);
    const hedge = await openBaseHedge(fixture);
    const baseYlpBefore = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpBefore = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapBaseForQuote(fixture, baseHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const baseYlpAfter = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpAfter = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseYlpAfter.amount < baseYlpBefore.amount).to.equal(true);
    expect(quoteYlpAfter.amount < quoteYlpBefore.amount).to.equal(true);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(10_000);
    expect(decoded.base_hlp_vault.ylp_base_shares.toNumber()).to.be.lessThan(10_000);
    expect(decoded.base_hlp_vault.ylp_quote_shares.toNumber()).to.be.lessThan(20_000);
  });

  it("checkpoints quote hLP vaults during opposite-direction swaps", async function () {
    const fixture = await addBalancedLiquidity(55);
    const hedge = await openQuoteHedge(fixture);
    const baseYlpBefore = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpBefore = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapQuoteForBase(fixture, quoteHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const baseYlpAfter = await getAccount(
      connection as any,
      hedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteYlpAfter = await getAccount(
      connection as any,
      hedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseYlpAfter.amount < baseYlpBefore.amount).to.equal(true);
    expect(quoteYlpAfter.amount < quoteYlpBefore.amount).to.equal(true);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(20_000);
    expect(decoded.quote_hlp_vault.ylp_base_shares.toNumber()).to.be.lessThan(10_000);
    expect(decoded.quote_hlp_vault.ylp_quote_shares.toNumber()).to.be.lessThan(20_000);
  });

  it("checkpoints both aggregate hLP vaults in one swap", async function () {
    const fixture = await addBalancedLiquidity(56);
    const baseHedge = await openBaseHedge(fixture);
    const quoteHedge = await openQuoteHedge(fixture);
    const baseHlpBaseYlpBefore = await getAccount(
      connection as any,
      baseHedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const baseHlpQuoteYlpBefore = await getAccount(
      connection as any,
      baseHedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpBaseYlpBefore = await getAccount(
      connection as any,
      quoteHedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpQuoteYlpBefore = await getAccount(
      connection as any,
      quoteHedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapBaseForQuote(fixture, allHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const baseHlpBaseYlpAfter = await getAccount(
      connection as any,
      baseHedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const baseHlpQuoteYlpAfter = await getAccount(
      connection as any,
      baseHedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpBaseYlpAfter = await getAccount(
      connection as any,
      quoteHedge.hlpBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpQuoteYlpAfter = await getAccount(
      connection as any,
      quoteHedge.hlpQuoteYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseHlpBaseYlpAfter.amount).to.not.equal(baseHlpBaseYlpBefore.amount);
    expect(baseHlpQuoteYlpAfter.amount).to.not.equal(baseHlpQuoteYlpBefore.amount);
    expect(quoteHlpBaseYlpAfter.amount).to.not.equal(quoteHlpBaseYlpBefore.amount);
    expect(quoteHlpQuoteYlpAfter.amount).to.not.equal(quoteHlpQuoteYlpBefore.amount);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(10_000);
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(20_000);
  });

  it("sets a yield recipient and claims non-compounding yLP swap fees", async function () {
    const fixture = await addBalancedLiquidity(48);
    const recipient = Keypair.generate().publicKey;
    const recipientBaseAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseMint,
      recipient
    );
    const baseYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.baseMint,
      "ylp"
    )[0];

    const setRecipientTx = await program.methods
      .setYieldRecipient({
        tokenKind: { ylp: {} },
        recipient,
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        yieldAccount: baseYieldAccount,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(setRecipientTx, [payer]);
    trackV2Instruction("setYieldRecipient", this.test?.title);

    await swapBaseForQuote(fixture);

    const claimTx = await program.methods
      .claimYield({
        marketAsset: { base: {} },
        tokenKind: { ylp: {} },
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        lpMint: fixture.baseYlpMint,
        ownerLpAccount: fixture.ownerBaseYlpAccount,
        feeVault: fixture.baseFeeVault,
        interestVault: fixture.baseInterestVault,
        recipientAssetAccount: recipientBaseAccount,
        yieldAccount: baseYieldAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(claimTx, [payer]);
    trackV2Instruction("claimYield", this.test?.title);

    const recipientBalance = await getAccount(connection as any, recipientBaseAccount);
    expect(recipientBalance.amount).to.equal(3n);
    const feeVault = await getAccount(connection as any, fixture.baseFeeVault);
    expect(feeVault.amount).to.equal(0n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.fees.swap_fee_liability.toNumber()).to.equal(0);
    expect(decoded.base_side.fees.swap_fee_vault_balance.toNumber()).to.equal(0);
  });

  it("checkpoints yLP yield accounts during a Token-2022 transfer hook", async function () {
    const fixture = await addBalancedLiquidity(58);
    const recipient = Keypair.generate().publicKey;
    const destinationBaseYlpAccount = await createAccount(
      connection as any,
      payer,
      fixture.baseYlpMint,
      recipient,
      Keypair.generate(),
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const sourceYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.baseMint,
      "ylp"
    )[0];
    const [destinationYieldAccount, destinationYieldBump] = deriveYieldAccountAddress(
      fixture.market,
      recipient,
      fixture.baseMint,
      "ylp"
    );
    const validationAccount = seedTransferHookValidationAccount(
      fixture.baseYlpMint,
      fixture.market,
      fixture.baseMint,
      "ylp"
    );

    const metas = buildYieldTransferHookAccountMetas({
      lpMint: fixture.baseYlpMint,
      market: fixture.market,
      sourceOwner: payer.publicKey,
      destinationOwner: recipient,
      assetMint: fixture.baseMint,
      tokenKind: "ylp",
    });

    expect(metas.map((meta) => meta.pubkey.toString())).to.deep.equal([
      fixture.market.toString(),
      fixture.baseMint.toString(),
      sourceYieldAccount.toString(),
      destinationYieldAccount.toString(),
      OMNIPAIR_V2_PROGRAM_ID.toString(),
      validationAccount.toString(),
    ]);
    expect(metas.map((meta) => meta.isWritable)).to.deep.equal([
      false,
      false,
      true,
      true,
      false,
      false,
    ]);
    const selfTransferMetas = buildYieldTransferHookAccountMetas({
      lpMint: fixture.baseYlpMint,
      market: fixture.market,
      sourceOwner: payer.publicKey,
      destinationOwner: payer.publicKey,
      assetMint: fixture.baseMint,
      tokenKind: "ylp",
    });
    expect(selfTransferMetas.map((meta) => meta.pubkey.toString())).to.deep.equal([
      fixture.market.toString(),
      fixture.baseMint.toString(),
      sourceYieldAccount.toString(),
      sourceYieldAccount.toString(),
      OMNIPAIR_V2_PROGRAM_ID.toString(),
      validationAccount.toString(),
    ]);

    await seedYieldAccount(
      destinationYieldAccount,
      recipient,
      fixture.market,
      fixture.baseMint,
      "ylp",
      destinationYieldBump
    );
    await swapBaseForQuote(fixture);

    const transferIx = await createTransferCheckedWithTransferHookInstruction(
      connection as any,
      fixture.ownerBaseYlpAccount,
      fixture.baseYlpMint,
      destinationBaseYlpAccount,
      payer.publicKey,
      BigInt(10_000),
      6,
      [],
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    await connection.sendTransaction(new Transaction().add(transferIx), [payer]);

    const sourceYieldData = svm.getAccount(sourceYieldAccount);
    const destinationYieldData = svm.getAccount(destinationYieldAccount);
    expect(sourceYieldData).to.not.equal(null);
    expect(destinationYieldData).to.not.equal(null);
    const sourceYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(sourceYieldData!.data)
    ) as any;
    const destinationYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(destinationYieldData!.data)
    ) as any;
    expect(sourceYield.accrued_swap_fee_amount.toNumber()).to.equal(3);
    expect(destinationYield.accrued_swap_fee_amount.toNumber()).to.equal(0);

    const sourceYlpAfter = await getAccount(
      connection as any,
      fixture.ownerBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const destinationYlpAfter = await getAccount(
      connection as any,
      destinationBaseYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(sourceYlpAfter.amount).to.equal(90_000n);
    expect(destinationYlpAfter.amount).to.equal(10_000n);
  });

  it("deposits collateral, borrows fixed quote debt, repays, and withdraws idle collateral", async function () {
    const fixture = await addBalancedLiquidity(49);
    const marginPosition = deriveMarginPositionAddress(fixture.market, payer.publicKey)[0];
    const ownerBaseBefore = await getAccount(connection as any, fixture.ownerBaseAccount);
    const ownerQuoteBefore = await getAccount(connection as any, fixture.ownerQuoteAccount);

    const depositTx = await program.methods
      .depositCollateral({
        marketAsset: { base: {} },
        depositAmount: new BN(10_000),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        collateralVault: fixture.baseCollateralVault,
        ownerAssetAccount: fixture.ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(depositTx, [payer]);
    trackV2Instruction("depositCollateral", this.test?.title);

    const borrowTx = await program.methods
      .borrow({
        borrowAsset: { quote: {} },
        borrowAmount: new BN(5_000),
        minDebtAmountOut: new BN(5_000),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        debtAssetMint: fixture.quoteMint,
        collateralAssetMint: fixture.baseMint,
        reserveVault: fixture.quoteReserveVault,
        ownerDebtAccount: fixture.ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(borrowTx, [payer]);
    trackV2Instruction("borrow", this.test?.title);

    let ownerBase = await getAccount(connection as any, fixture.ownerBaseAccount);
    let ownerQuote = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerBase.amount).to.equal(ownerBaseBefore.amount - 10_000n);
    expect(ownerQuote.amount).to.equal(ownerQuoteBefore.amount + 5_000n);

    let positionAccount = svm.getAccount(marginPosition);
    expect(positionAccount).to.not.equal(null);
    let position = accountCoder.decode("MarginPosition", Buffer.from(positionAccount!.data)) as any;
    expect(position.base_collateral.toNumber()).to.equal(10_000);
    expect(position.fixed_quote_shares.toNumber()).to.equal(5_000);
    expect(position.recognized_base_collateral_for_quote_debt.toNumber()).to.be.greaterThan(0);

    const repayTx = await program.methods
      .repay({
        repayAsset: { quote: {} },
        repayAmount: new BN(5_000),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        debtAssetMint: fixture.quoteMint,
        reserveVault: fixture.quoteReserveVault,
        interestVault: fixture.quoteInterestVault,
        ownerDebtAccount: fixture.ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(repayTx, [payer]);
    trackV2Instruction("repay", this.test?.title);

    const withdrawTx = await program.methods
      .withdrawCollateral({
        marketAsset: { base: {} },
        withdrawAmount: new BN(10_000),
        minAssetAmountOut: new BN(10_000),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        collateralVault: fixture.baseCollateralVault,
        ownerAssetAccount: fixture.ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(withdrawTx, [payer]);
    trackV2Instruction("withdrawCollateral", this.test?.title);

    ownerBase = await getAccount(connection as any, fixture.ownerBaseAccount);
    ownerQuote = await getAccount(connection as any, fixture.ownerQuoteAccount);
    expect(ownerBase.amount).to.equal(ownerBaseBefore.amount);
    expect(ownerQuote.amount).to.equal(ownerQuoteBefore.amount);

    positionAccount = svm.getAccount(marginPosition);
    expect(positionAccount).to.not.equal(null);
    position = accountCoder.decode("MarginPosition", Buffer.from(positionAccount!.data)) as any;
    expect(position.base_collateral.toNumber()).to.equal(0);
    expect(position.fixed_quote_shares.toNumber()).to.equal(0);
    expect(position.recognized_base_collateral_for_quote_debt.toNumber()).to.equal(0);

    const decoded = accountCoder.decode(
      "Market",
      Buffer.from(svm.getAccount(fixture.market)!.data)
    ) as any;
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.quote_side.reserves.cash_reserve.toNumber()).to.equal(200_000);
    expect(decoded.debt.fixed_quote_shares.toNumber()).to.equal(0);
  });

  it("liquidates unhealthy fixed quote debt after collateral price moves", async function () {
    const liquidationConfig = marketConfig();
    liquidationConfig.spotEmaDivergenceBps = 10_000;
    const fixture = await addBalancedLiquidity(54, liquidationConfig);
    const marginPosition = deriveMarginPositionAddress(fixture.market, payer.publicKey)[0];

    const depositTx = await program.methods
      .depositCollateral({
        marketAsset: { base: {} },
        depositAmount: new BN(10_000),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        collateralVault: fixture.baseCollateralVault,
        ownerAssetAccount: fixture.ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(depositTx, [payer]);

    const borrowTx = await program.methods
      .borrow({
        borrowAsset: { quote: {} },
        borrowAmount: new BN(14_500),
        minDebtAmountOut: new BN(14_500),
        minHealthBps: new BN(11_000),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        debtAssetMint: fixture.quoteMint,
        collateralAssetMint: fixture.baseMint,
        reserveVault: fixture.quoteReserveVault,
        ownerDebtAccount: fixture.ownerQuoteAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(borrowTx, [payer]);

    await swapBaseForQuote(fixture, [], 5_000, 8_500);

    const positionBeforeAccount = svm.getAccount(marginPosition);
    expect(positionBeforeAccount).to.not.equal(null);
    const positionBefore = accountCoder.decode(
      "MarginPosition",
      Buffer.from(positionBeforeAccount!.data)
    ) as any;
    const baseCollateralBefore = positionBefore.base_collateral.toNumber();
    const quoteDebtSharesBefore = BigInt(positionBefore.fixed_quote_shares.toString());
    const ownerBaseBefore = await getAccount(connection as any, fixture.ownerBaseAccount);

    const liquidateTx = await program.methods
      .liquidate({
        debtAsset: { quote: {} },
        repayAmount: new BN(1_000),
        minCollateralOut: new BN(1),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market: fixture.market,
        liquidator: payer.publicKey,
        debtAssetMint: fixture.quoteMint,
        collateralAssetMint: fixture.baseMint,
        reserveVault: fixture.quoteReserveVault,
        interestVault: fixture.quoteInterestVault,
        collateralVault: fixture.baseCollateralVault,
        insuranceVault: fixture.quoteInsuranceVault,
        collateralInsuranceVault: fixture.baseInsuranceVault,
        liquidatorDebtAccount: fixture.ownerQuoteAccount,
        liquidatorCollateralAccount: fixture.ownerBaseAccount,
        marginPosition,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: eventAuthority(),
        program: OMNIPAIR_V2_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(liquidateTx, [payer]);
    trackV2Instruction("liquidate", this.test?.title);

    const ownerBaseAfter = await getAccount(connection as any, fixture.ownerBaseAccount);
    expect(ownerBaseAfter.amount > ownerBaseBefore.amount).to.equal(true);

    const positionAfterAccount = svm.getAccount(marginPosition);
    expect(positionAfterAccount).to.not.equal(null);
    const positionAfter = accountCoder.decode(
      "MarginPosition",
      Buffer.from(positionAfterAccount!.data)
    ) as any;
    expect(positionAfter.base_collateral.toNumber()).to.be.lessThan(baseCollateralBefore);
    expect(BigInt(positionAfter.fixed_quote_shares.toString()) < quoteDebtSharesBefore).to.equal(
      true
    );
  });
});
