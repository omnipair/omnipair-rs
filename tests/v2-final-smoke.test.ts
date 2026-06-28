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
  buildYlpTransferHookAccountMetas,
  buildYlpTransferHookValidationAccountData,
  deriveFutarchyAuthorityV2Address,
  deriveHlpYlpVaultAddress,
  deriveInsuranceAddress,
  deriveMarketAddress,
  deriveMarketCollateralVaultAddress,
  deriveMarketFeeVaultAddress,
  deriveMarketInterestVaultAddress,
  deriveMarketReserveVaultAddress,
  deriveLiquidationAuctionAddress,
  deriveMarginPositionAddress,
  deriveYieldAccountAddress,
  deriveYieldTransferHookValidationAddress,
  deriveTokenMetadataAddress,
  TOKEN_METADATA_PROGRAM_ID,
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
const RUN_REAL_TOKEN_METADATA_CPI = process.env.OMNIPAIR_V2_TEST_REAL_METADATA_CPI === "1";

function tokenMetadataProgramPath() {
  const fallback =
    "/Users/User/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/trident-svm-0.2.0/src/solana-program-library/metaplex-token-metadata.so";
  if (fs.existsSync(fallback)) return fallback;

  const depsDir = path.join(__dirname, "../target/sbpf-solana-solana/release/deps");
  if (fs.existsSync(depsDir)) {
    const candidate = fs
      .readdirSync(depsDir)
      .filter((name) => name.startsWith("mpl_token_metadata-") && name.endsWith(".so"))
      .sort()[0];
    if (candidate) return path.join(depsDir, candidate);
  }
  throw new Error("Token Metadata program file not found");
}

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
    liquidationAuctionDurationSlots: new BN(1_200),
    liquidationAuctionStartIncentiveBps: 0,
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
    if (RUN_REAL_TOKEN_METADATA_CPI) {
      svm.addProgramFromFile(TOKEN_METADATA_PROGRAM_ID, tokenMetadataProgramPath());
    }
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

  function seedYlpTransferHookValidationAccount(
    lpMint: PublicKey,
    market: PublicKey,
    baseMint: PublicKey,
    quoteMint: PublicKey
  ) {
    const validationAccount = deriveYieldTransferHookValidationAddress(lpMint)[0];
    const data = buildYlpTransferHookValidationAccountData({
      market,
      baseMint,
      quoteMint,
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
    if (RUN_REAL_TOKEN_METADATA_CPI) {
      isolatedSvm.addProgramFromFile(TOKEN_METADATA_PROGRAM_ID, tokenMetadataProgramPath());
    }
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
    const ylpMint = await createHookedLpMint(market, 6);
    const baseHlpMint = await createHookedLpMint(market, 6);
    const quoteHlpMint = await createHookedLpMint(market, 6);
    const ylpTokenMetadata = deriveTokenMetadataAddress(ylpMint)[0];
    const baseHlpTokenMetadata = deriveTokenMetadataAddress(baseHlpMint)[0];
    const quoteHlpTokenMetadata = deriveTokenMetadataAddress(quoteHlpMint)[0];
    const baseHlpYlpVault = deriveHlpYlpVaultAddress(market, baseHlpMint, ylpMint)[0];
    const quoteHlpYlpVault = deriveHlpYlpVaultAddress(market, quoteHlpMint, ylpMint)[0];
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
        ylpMint,
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

    await initializeLpMetadata({
      market,
      lpMint: ylpMint,
      lpTokenMetadata: ylpTokenMetadata,
      name: "Omnipair Dusk yLP",
      symbol: "yLP",
      uri: "https://omnipair.fi/metadata/dusk/ylp.json",
    });
    await initializeLpMetadata({
      market,
      lpMint: baseHlpMint,
      lpTokenMetadata: baseHlpTokenMetadata,
      name: "Omnipair Dusk Base hLP",
      symbol: "hLP",
      uri: "https://omnipair.fi/metadata/dusk/base-hlp.json",
    });
    await initializeLpMetadata({
      market,
      lpMint: quoteHlpMint,
      lpTokenMetadata: quoteHlpTokenMetadata,
      name: "Omnipair Dusk Quote hLP",
      symbol: "hLP",
      uri: "https://omnipair.fi/metadata/dusk/quote-hlp.json",
    });
    return {
      baseMint,
      quoteMint,
      paramsHash,
      market,
      ylpMint,
      baseHlpMint,
      quoteHlpMint,
      ylpTokenMetadata,
      baseHlpTokenMetadata,
      quoteHlpTokenMetadata,
      baseHlpYlpVault,
      quoteHlpYlpVault,
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

  async function initializeLpMetadata(params: {
    market: PublicKey;
    lpMint: PublicKey;
    lpTokenMetadata: PublicKey;
    name: string;
    symbol: string;
    uri: string;
  }) {
    if (!RUN_REAL_TOKEN_METADATA_CPI) {
      const data = Buffer.from(
        JSON.stringify({
          name: params.name,
          symbol: params.symbol,
          uri: params.uri,
        })
      );
      svm.setAccount(params.lpTokenMetadata, {
        lamports: Number(svm.minimumBalanceForRentExemption(BigInt(data.length))),
        data: new Uint8Array(data),
        owner: TOKEN_METADATA_PROGRAM_ID,
        executable: false,
        rentEpoch: 0,
      });
      return;
    }

    const tx = await program.methods
      .initializeLpMetadata({
        name: params.name,
        symbol: params.symbol,
        uri: params.uri,
      })
      .accounts({
        payer: payer.publicKey,
        market: params.market,
        lpMint: params.lpMint,
        lpTokenMetadata: params.lpTokenMetadata,
        systemProgram: SystemProgram.programId,
        tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .transaction();
    await connection.sendTransaction(tx, [payer]);
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
    const ownerYlpAccount = await createAccount(
      connection as any,
      payer,
      fixture.ylpMint,
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
      ownerYlpAccount,
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
        minYlpAmount: new BN(100_000),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerBaseAccount: ownerAccounts.ownerBaseAccount,
        ownerQuoteAccount: ownerAccounts.ownerQuoteAccount,
        ownerYlpAccount: ownerAccounts.ownerYlpAccount,
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
    const hlpYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      fixture.baseHlpMint,
      fixture.ylpMint
    )[0];
    const targetYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.baseMint,
      "hlp"
    )[0];

    const tx = await program.methods
      .openHedge({
        depositAmount: new BN(depositAmount),
        minHlpAmount: new BN(1),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        targetHlpMint: fixture.baseHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerTargetAccount: fixture.ownerBaseAccount,
        ownerHlpAccount: ownerBaseHlpAccount,
        hlpYlpAccount,
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
      hlpYlpAccount,
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
    const hlpYlpAccount = deriveHlpYlpVaultAddress(
      fixture.market,
      fixture.quoteHlpMint,
      fixture.ylpMint
    )[0];
    const targetYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.quoteMint,
      "hlp"
    )[0];

    const tx = await program.methods
      .openHedge({
        depositAmount: new BN(depositAmount),
        minHlpAmount: new BN(1),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        targetHlpMint: fixture.quoteHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerTargetAccount: fixture.ownerQuoteAccount,
        ownerHlpAccount: ownerQuoteHlpAccount,
        hlpYlpAccount,
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
      hlpYlpAccount,
      targetYieldAccount,
    };
  }

  function baseHlpRebalanceAccounts(fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>) {
    return [
      {
        pubkey: fixture.ylpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.baseHlpYlpVault,
        isWritable: true,
        isSigner: false,
      },
    ];
  }

  function quoteHlpRebalanceAccounts(fixture: Awaited<ReturnType<typeof addBalancedLiquidity>>) {
    return [
      {
        pubkey: fixture.ylpMint,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: fixture.quoteHlpYlpVault,
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
    expect(decoded.ylp_mint.toString()).to.equal(fixture.ylpMint.toString());
    expect(decoded.base_side.hlp_mint.toString()).to.equal(fixture.baseHlpMint.toString());
    expect(decoded.quote_side.hlp_mint.toString()).to.equal(fixture.quoteHlpMint.toString());
    expect(decoded.base_hlp_vault.ylp_vault.toString()).to.equal(
      fixture.baseHlpYlpVault.toString()
    );
    expect(decoded.quote_hlp_vault.ylp_vault.toString()).to.equal(
      fixture.quoteHlpYlpVault.toString()
    );
    expect(svm.getAccount(fixture.ylpTokenMetadata)).to.not.equal(null);
    expect(svm.getAccount(fixture.baseHlpTokenMetadata)).to.not.equal(null);
    expect(svm.getAccount(fixture.quoteHlpTokenMetadata)).to.not.equal(null);
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

    const ylpAccount = await getAccount(
      connection as any,
      fixture.ownerYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ylpAccount.amount).to.equal(100_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(100_000);
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
    const vaultYlp = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(10_000n);
    expect(vaultYlp.amount).to.equal(10_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(110_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(220_000);
    expect(decoded.base_hlp_vault.ylp_shares.toNumber()).to.equal(10_000);
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
    const vaultYlp = await getAccount(
      connection as any,
      first.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(11_000n);
    expect(vaultYlp.amount).to.equal(11_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_hlp_vault.ylp_shares.toNumber()).to.equal(11_000);
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(11_000);
  });

  it("closes base hLP by burning vault yLP, repaying quote debt, and returning base", async function () {
    const fixture = await addBalancedLiquidity(45);
    const ownerBaseBeforeOpen = await getAccount(connection as any, fixture.ownerBaseAccount);
    const hedge = await openBaseHedge(fixture);

    const tx = await program.methods
      .closeHedge({
        hlpAmount: new BN(10_000),
        minTargetAmountOut: new BN(9_999),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        targetHlpMint: fixture.baseHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        borrowedInterestVault: fixture.quoteInterestVault,
        ownerTargetAccount: fixture.ownerBaseAccount,
        ownerHlpAccount: hedge.ownerBaseHlpAccount,
        hlpYlpAccount: hedge.hlpYlpAccount,
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
    const vaultYlp = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(0n);
    expect(vaultYlp.amount).to.equal(0n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(200_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(100_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(100_000);
    expect(decoded.base_hlp_vault.ylp_shares.toNumber()).to.equal(0);
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
    const vaultYlp = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ownerHlp.amount).to.equal(20_000n);
    expect(vaultYlp.amount).to.equal(10_000n);

    let account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    let decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.quote_hlp_vault.ylp_shares.toNumber()).to.equal(10_000);
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(20_000);
    expect(decoded.quote_hlp_vault.debt_shares.toNumber()).to.be.greaterThan(0);

    const tx = await program.methods
      .closeHedge({
        hlpAmount: new BN(20_000),
        minTargetAmountOut: new BN(19_999),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        targetHlpMint: fixture.quoteHlpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        borrowedInterestVault: fixture.baseInterestVault,
        ownerTargetAccount: fixture.ownerQuoteAccount,
        ownerHlpAccount: hedge.ownerQuoteHlpAccount,
        hlpYlpAccount: hedge.hlpYlpAccount,
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
    expect(decoded.quote_hlp_vault.ylp_shares.toNumber()).to.equal(0);
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(0);
    expect(decoded.quote_hlp_vault.debt_shares.toNumber()).to.equal(0);
  });

  it("removes matched yLP liquidity and returns pro-rata reserves", async function () {
    const fixture = await addBalancedLiquidity(46);
    const ownerBaseBefore = await getAccount(connection as any, fixture.ownerBaseAccount);
    const ownerQuoteBefore = await getAccount(connection as any, fixture.ownerQuoteAccount);

    const tx = await program.methods
      .removeLiquidity({
        ylpAmount: new BN(1_000),
        minBaseAmountOut: new BN(1_000),
        minQuoteAmountOut: new BN(2_000),
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        baseMint: fixture.baseMint,
        quoteMint: fixture.quoteMint,
        ylpMint: fixture.ylpMint,
        baseReserveVault: fixture.baseReserveVault,
        quoteReserveVault: fixture.quoteReserveVault,
        ownerBaseAccount: fixture.ownerBaseAccount,
        ownerQuoteAccount: fixture.ownerQuoteAccount,
        ownerYlpAccount: fixture.ownerYlpAccount,
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

    const ylpAccount = await getAccount(
      connection as any,
      fixture.ownerYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ylpAccount.amount).to.equal(99_000n);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_side.reserves.live_reserve.toNumber()).to.equal(99_000);
    expect(decoded.quote_side.reserves.live_reserve.toNumber()).to.equal(198_000);
    expect(decoded.base_side.shares.ylp_supply.toNumber()).to.equal(99_000);
    expect(decoded.quote_side.shares.ylp_supply.toNumber()).to.equal(99_000);
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
    const ylpBefore = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapBaseForQuote(fixture, baseHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const ylpAfter = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ylpAfter.amount < ylpBefore.amount).to.equal(true);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.base_hlp_vault.hlp_supply.toNumber()).to.equal(10_000);
    expect(decoded.base_hlp_vault.ylp_shares.toNumber()).to.be.lessThan(10_000);
  });

  it("checkpoints quote hLP vaults during opposite-direction swaps", async function () {
    const fixture = await addBalancedLiquidity(55);
    const hedge = await openQuoteHedge(fixture);
    const ylpBefore = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapQuoteForBase(fixture, quoteHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const ylpAfter = await getAccount(
      connection as any,
      hedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(ylpAfter.amount < ylpBefore.amount).to.equal(true);

    const account = svm.getAccount(fixture.market);
    expect(account).to.not.equal(null);
    const decoded = accountCoder.decode("Market", Buffer.from(account!.data)) as any;
    expect(decoded.quote_hlp_vault.hlp_supply.toNumber()).to.equal(20_000);
    expect(decoded.quote_hlp_vault.ylp_shares.toNumber()).to.be.lessThan(10_000);
  });

  it("checkpoints both aggregate hLP vaults in one swap", async function () {
    const fixture = await addBalancedLiquidity(56);
    const baseHedge = await openBaseHedge(fixture);
    const quoteHedge = await openQuoteHedge(fixture);
    const baseHlpYlpBefore = await getAccount(
      connection as any,
      baseHedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpYlpBefore = await getAccount(
      connection as any,
      quoteHedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await swapBaseForQuote(fixture, allHlpRebalanceAccounts(fixture));
    trackV2Instruction("swap", this.test?.title);

    const baseHlpYlpAfter = await getAccount(
      connection as any,
      baseHedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const quoteHlpYlpAfter = await getAccount(
      connection as any,
      quoteHedge.hlpYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    expect(baseHlpYlpAfter.amount).to.not.equal(baseHlpYlpBefore.amount);
    expect(quoteHlpYlpAfter.amount).to.not.equal(quoteHlpYlpBefore.amount);

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
        tokenKind: { ylp: {} },
      })
      .accounts({
        market: fixture.market,
        owner: payer.publicKey,
        assetMint: fixture.baseMint,
        lpMint: fixture.ylpMint,
        ownerLpAccount: fixture.ownerYlpAccount,
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
    const destinationYlpAccount = await createAccount(
      connection as any,
      payer,
      fixture.ylpMint,
      recipient,
      Keypair.generate(),
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const sourceBaseYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.baseMint,
      "ylp"
    )[0];
    const [destinationBaseYieldAccount, destinationBaseYieldBump] = deriveYieldAccountAddress(
      fixture.market,
      recipient,
      fixture.baseMint,
      "ylp"
    );
    const sourceQuoteYieldAccount = deriveYieldAccountAddress(
      fixture.market,
      payer.publicKey,
      fixture.quoteMint,
      "ylp"
    )[0];
    const [destinationQuoteYieldAccount, destinationQuoteYieldBump] = deriveYieldAccountAddress(
      fixture.market,
      recipient,
      fixture.quoteMint,
      "ylp"
    );
    const validationAccount = seedYlpTransferHookValidationAccount(
      fixture.ylpMint,
      fixture.market,
      fixture.baseMint,
      fixture.quoteMint
    );

    const metas = buildYlpTransferHookAccountMetas({
      lpMint: fixture.ylpMint,
      market: fixture.market,
      sourceOwner: payer.publicKey,
      destinationOwner: recipient,
      baseMint: fixture.baseMint,
      quoteMint: fixture.quoteMint,
    });

    expect(metas.map((meta) => meta.pubkey.toString())).to.deep.equal([
      fixture.market.toString(),
      fixture.baseMint.toString(),
      fixture.quoteMint.toString(),
      sourceBaseYieldAccount.toString(),
      destinationBaseYieldAccount.toString(),
      sourceQuoteYieldAccount.toString(),
      destinationQuoteYieldAccount.toString(),
      OMNIPAIR_V2_PROGRAM_ID.toString(),
      validationAccount.toString(),
    ]);
    expect(metas.map((meta) => meta.isWritable)).to.deep.equal([
      false,
      false,
      false,
      true,
      true,
      true,
      true,
      false,
      false,
    ]);
    const selfTransferMetas = buildYlpTransferHookAccountMetas({
      lpMint: fixture.ylpMint,
      market: fixture.market,
      sourceOwner: payer.publicKey,
      destinationOwner: payer.publicKey,
      baseMint: fixture.baseMint,
      quoteMint: fixture.quoteMint,
    });
    expect(selfTransferMetas.map((meta) => meta.pubkey.toString())).to.deep.equal([
      fixture.market.toString(),
      fixture.baseMint.toString(),
      fixture.quoteMint.toString(),
      sourceBaseYieldAccount.toString(),
      sourceBaseYieldAccount.toString(),
      sourceQuoteYieldAccount.toString(),
      sourceQuoteYieldAccount.toString(),
      OMNIPAIR_V2_PROGRAM_ID.toString(),
      validationAccount.toString(),
    ]);

    await seedYieldAccount(
      destinationBaseYieldAccount,
      recipient,
      fixture.market,
      fixture.baseMint,
      "ylp",
      destinationBaseYieldBump
    );
    await seedYieldAccount(
      destinationQuoteYieldAccount,
      recipient,
      fixture.market,
      fixture.quoteMint,
      "ylp",
      destinationQuoteYieldBump
    );
    await swapBaseForQuote(fixture);

    const transferIx = await createTransferCheckedWithTransferHookInstruction(
      connection as any,
      fixture.ownerYlpAccount,
      fixture.ylpMint,
      destinationYlpAccount,
      payer.publicKey,
      BigInt(10_000),
      6,
      [],
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    await connection.sendTransaction(new Transaction().add(transferIx), [payer]);

    const sourceBaseYieldData = svm.getAccount(sourceBaseYieldAccount);
    const destinationBaseYieldData = svm.getAccount(destinationBaseYieldAccount);
    const sourceQuoteYieldData = svm.getAccount(sourceQuoteYieldAccount);
    const destinationQuoteYieldData = svm.getAccount(destinationQuoteYieldAccount);
    expect(sourceBaseYieldData).to.not.equal(null);
    expect(destinationBaseYieldData).to.not.equal(null);
    expect(sourceQuoteYieldData).to.not.equal(null);
    expect(destinationQuoteYieldData).to.not.equal(null);
    const sourceBaseYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(sourceBaseYieldData!.data)
    ) as any;
    const destinationBaseYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(destinationBaseYieldData!.data)
    ) as any;
    const sourceQuoteYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(sourceQuoteYieldData!.data)
    ) as any;
    const destinationQuoteYield = accountCoder.decode(
      "YieldAccount",
      Buffer.from(destinationQuoteYieldData!.data)
    ) as any;
    expect(sourceBaseYield.accrued_swap_fee_amount.toNumber()).to.equal(3);
    expect(destinationBaseYield.accrued_swap_fee_amount.toNumber()).to.equal(0);
    expect(sourceQuoteYield.accrued_swap_fee_amount.toNumber()).to.equal(0);
    expect(destinationQuoteYield.accrued_swap_fee_amount.toNumber()).to.equal(0);

    const sourceYlpAfter = await getAccount(
      connection as any,
      fixture.ownerYlpAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const destinationYlpAfter = await getAccount(
      connection as any,
      destinationYlpAccount,
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
    const liquidationAuction = deriveLiquidationAuctionAddress(
      fixture.market,
      marginPosition,
      fixture.quoteMint
    )[0];

    const openAuctionTx = await program.methods
      .openLiquidationAuction()
      .accounts({
        keeper: payer.publicKey,
        market: fixture.market,
        debtAssetMint: fixture.quoteMint,
        collateralAssetMint: fixture.baseMint,
        marginPosition,
        liquidationAuction,
        systemProgram: SystemProgram.programId,
      })
      .transaction();
    await connection.sendTransaction(openAuctionTx, [payer]);
    trackV2Instruction("openLiquidationAuction", this.test?.title);

    const settleAuctionTx = await program.methods
      .settleLiquidationAuction({
        repayAmount: new BN(1),
        minCollateralOut: new BN(1),
        maxInsuranceDraw: new BN(0),
        maxSocializedLoss: new BN(0),
      })
      .accounts({
        market: fixture.market,
        futarchyAuthority,
        bidder: payer.publicKey,
        debtAssetMint: fixture.quoteMint,
        collateralAssetMint: fixture.baseMint,
        reserveVault: fixture.quoteReserveVault,
        interestVault: fixture.quoteInterestVault,
        collateralVault: fixture.baseCollateralVault,
        insuranceVault: fixture.quoteInsuranceVault,
        collateralInsuranceVault: fixture.baseInsuranceVault,
        bidderDebtAccount: fixture.ownerQuoteAccount,
        bidderCollateralAccount: fixture.ownerBaseAccount,
        marginPosition,
        liquidationAuction,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .transaction();
    await connection.sendTransaction(settleAuctionTx, [payer]);
    trackV2Instruction("settleLiquidationAuction", this.test?.title);

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
