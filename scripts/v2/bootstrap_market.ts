import anchor from "@coral-xyz/anchor";
import {
  NATIVE_MINT,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  PublicKey,
  SystemProgram,
  bnFromUnits,
  createHookedLpMintIfMissing,
  defaultMarketConfig,
  deriveFutarchyAuthorityAddress,
  deriveHlpYlpVaultAddress,
  deriveMarketAddresses,
  deriveProgramDataAddress,
  deriveYieldAccountAddress,
  explorerTx,
  getOrCreateAta,
  mintDecimals,
  mintMockTokens,
  orderedMints,
  paramsHashForMarket,
  parseUnits,
  payerFromProvider,
  providerFromEnv,
  readState,
  tokenProgramForMint,
  v2Program,
  writeState,
} from "./common.ts";
import idl from "../../target/idl/omnipair_v2.json" with { type: "json" };

async function main() {
  const provider = providerFromEnv();
  const payer = payerFromProvider(provider);
  const program = v2Program(idl, provider);
  const state = readState();
  const baseLabel = process.env.OMNIPAIR_V2_MARKET_BASE_LABEL ?? "base";
  const quoteLabel = process.env.OMNIPAIR_V2_MARKET_QUOTE_LABEL ?? "quote";
  const marketLabel = process.env.OMNIPAIR_V2_MARKET_LABEL ?? `${baseLabel}-${quoteLabel}`;
  const storedBaseMint = state.mockMints[baseLabel];
  const storedQuoteMint = state.mockMints[quoteLabel];

  if (!storedBaseMint || !storedQuoteMint) {
    throw new Error("Mock mints are missing. Run yarn v2:create-mock-tokens first.");
  }

  const [baseMint, quoteMint] = orderedMints(
    new PublicKey(storedBaseMint.mint),
    new PublicKey(storedQuoteMint.mint)
  );
  const baseDecimals = await mintDecimals(provider.connection, baseMint);
  const quoteDecimals = await mintDecimals(provider.connection, quoteMint);
  const paramsHash = paramsHashForMarket(marketLabel, baseMint, quoteMint);
  const addresses = deriveMarketAddresses({
    programId: program.programId,
    baseMint,
    quoteMint,
    paramsHash,
  });
  const market = addresses.market;

  const futarchyAuthority = deriveFutarchyAuthorityAddress(program.programId);
  const futarchy = await ensureFutarchyAuthority({
    program,
    payer: payer.publicKey,
    futarchyAuthority,
  });
  const teamTreasury = futarchy.recipients.teamTreasury as PublicKey;
  const teamTreasuryWsolAccount = await getOrCreateAta({
    connection: provider.connection,
    payer,
    mint: NATIVE_MINT,
    owner: teamTreasury,
    tokenProgram: TOKEN_PROGRAM_ID,
  });

  const baseYlpMint = await createHookedLpMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-base-ylp`,
    decimals: baseDecimals,
    mintAuthority: market,
    transferHookProgramId: program.programId,
  });
  const quoteYlpMint = await createHookedLpMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-quote-ylp`,
    decimals: quoteDecimals,
    mintAuthority: market,
    transferHookProgramId: program.programId,
  });
  const baseHlpMint = await createHookedLpMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-base-hlp`,
    decimals: baseDecimals,
    mintAuthority: market,
    transferHookProgramId: program.programId,
  });
  const quoteHlpMint = await createHookedLpMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-quote-hlp`,
    decimals: quoteDecimals,
    mintAuthority: market,
    transferHookProgramId: program.programId,
  });

  const baseYlp = new PublicKey(baseYlpMint.mint);
  const quoteYlp = new PublicKey(quoteYlpMint.mint);
  const baseHlpBaseYlpVault = deriveHlpYlpVaultAddress(
    program.programId,
    market,
    "base",
    baseYlp
  );
  const baseHlpQuoteYlpVault = deriveHlpYlpVaultAddress(
    program.programId,
    market,
    "base",
    quoteYlp
  );
  const quoteHlpBaseYlpVault = deriveHlpYlpVaultAddress(
    program.programId,
    market,
    "quote",
    baseYlp
  );
  const quoteHlpQuoteYlpVault = deriveHlpYlpVaultAddress(
    program.programId,
    market,
    "quote",
    quoteYlp
  );

  const marketAccount = await provider.connection.getAccountInfo(market, "confirmed");
  if (!marketAccount) {
    console.log(`Initializing V2 yLP/hLP market ${market.toBase58()}`);
    const signature = await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: futarchy.authority,
        config: defaultMarketConfig(),
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        baseMint,
        quoteMint,
        market,
        futarchyAuthority,
        baseYlpMint: baseYlp,
        quoteYlpMint: quoteYlp,
        baseHlpMint: new PublicKey(baseHlpMint.mint),
        quoteHlpMint: new PublicKey(quoteHlpMint.mint),
        baseReserveVault: addresses.baseReserveVault,
        quoteReserveVault: addresses.quoteReserveVault,
        baseCollateralVault: addresses.baseCollateralVault,
        quoteCollateralVault: addresses.quoteCollateralVault,
        baseInsuranceVault: addresses.baseInsuranceVault,
        quoteInsuranceVault: addresses.quoteInsuranceVault,
        baseFeeVault: addresses.baseFeeVault,
        quoteFeeVault: addresses.quoteFeeVault,
        baseInterestVault: addresses.baseInterestVault,
        quoteInterestVault: addresses.quoteInterestVault,
        teamTreasury,
        teamTreasuryWsolAccount: teamTreasuryWsolAccount.address,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: addresses.eventAuthority,
        program: program.programId,
      })
      .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();
    console.log(`Initialize tx: ${explorerTx(signature)}`);
  } else {
    console.log(`Market already exists: ${market.toBase58()}`);
  }

  const storedMarket = {
    label: marketLabel,
    programId: program.programId.toBase58(),
    market: market.toBase58(),
    paramsHash: paramsHash.toString("hex"),
    baseMint: baseMint.toBase58(),
    quoteMint: quoteMint.toBase58(),
    baseYlpMint: baseYlpMint.mint,
    quoteYlpMint: quoteYlpMint.mint,
    baseHlpMint: baseHlpMint.mint,
    quoteHlpMint: quoteHlpMint.mint,
    baseReserveVault: addresses.baseReserveVault.toBase58(),
    quoteReserveVault: addresses.quoteReserveVault.toBase58(),
    baseCollateralVault: addresses.baseCollateralVault.toBase58(),
    quoteCollateralVault: addresses.quoteCollateralVault.toBase58(),
    baseInsuranceVault: addresses.baseInsuranceVault.toBase58(),
    quoteInsuranceVault: addresses.quoteInsuranceVault.toBase58(),
    baseFeeVault: addresses.baseFeeVault.toBase58(),
    quoteFeeVault: addresses.quoteFeeVault.toBase58(),
    baseInterestVault: addresses.baseInterestVault.toBase58(),
    quoteInterestVault: addresses.quoteInterestVault.toBase58(),
    baseHlpBaseYlpVault: baseHlpBaseYlpVault.toBase58(),
    baseHlpQuoteYlpVault: baseHlpQuoteYlpVault.toBase58(),
    quoteHlpBaseYlpVault: quoteHlpBaseYlpVault.toBase58(),
    quoteHlpQuoteYlpVault: quoteHlpQuoteYlpVault.toBase58(),
    eventAuthority: addresses.eventAuthority.toBase58(),
    seededLiquidity:
      state.markets[marketLabel]?.market === market.toBase58()
        ? state.markets[marketLabel]?.seededLiquidity ?? false
        : false,
  };
  state.markets[marketLabel] = storedMarket;
  writeState(state);

  const shouldSeed =
    process.env.OMNIPAIR_V2_SEED_LIQUIDITY !== "0" &&
    (!storedMarket.seededLiquidity || process.env.OMNIPAIR_V2_FORCE_SEED === "1");
  if (!shouldSeed) {
    console.log("Skipping reserve seeding");
    console.log(JSON.stringify(storedMarket, null, 2));
    return;
  }

  const baseAmount = parseUnits(process.env.OMNIPAIR_V2_BASE_LIQUIDITY ?? "100000", baseDecimals);
  const quoteAmount = parseUnits(
    process.env.OMNIPAIR_V2_QUOTE_LIQUIDITY ?? "100000",
    quoteDecimals
  );
  await seedBalancedLiquidity({
    provider,
    payer,
    program,
    market,
    futarchyAuthority,
    eventAuthority: addresses.eventAuthority,
    baseMint,
    quoteMint,
    baseYlpMint: baseYlp,
    quoteYlpMint: quoteYlp,
    baseReserveVault: addresses.baseReserveVault,
    quoteReserveVault: addresses.quoteReserveVault,
    baseAmount,
    quoteAmount,
  });

  state.markets[marketLabel] = {
    ...state.markets[marketLabel],
    seededLiquidity: true,
  };
  writeState(state);
  console.log("V2 yLP/hLP market bootstrap complete");
  console.log(JSON.stringify(state.markets[marketLabel], null, 2));
}

async function ensureFutarchyAuthority(params: {
  program: any;
  payer: PublicKey;
  futarchyAuthority: PublicKey;
}) {
  const existing = await params.program.account.futarchyAuthority.fetchNullable(
    params.futarchyAuthority
  );
  if (existing) {
    console.log(`Futarchy authority already exists: ${params.futarchyAuthority.toBase58()}`);
    return existing;
  }

  console.log(`Initializing V2 futarchy authority ${params.futarchyAuthority.toBase58()}`);
  const signature = await params.program.methods
    .initFutarchyAuthority({
      authority: params.payer,
      swapBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_SWAP_BPS ?? "0"),
      interestBps: Number(process.env.OMNIPAIR_V2_PROTOCOL_INTEREST_BPS ?? "0"),
      futarchyTreasury: params.payer,
      futarchyTreasuryBps: 0,
      buybacksVault: params.payer,
      buybacksVaultBps: 0,
      teamTreasury: params.payer,
      teamTreasuryBps: 10_000,
    })
    .accounts({
      deployer: params.payer,
      futarchyAuthority: params.futarchyAuthority,
      programData: deriveProgramDataAddress(params.program.programId),
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log(`Futarchy init tx: ${explorerTx(signature)}`);
  return await params.program.account.futarchyAuthority.fetch(params.futarchyAuthority);
}

async function seedBalancedLiquidity(params: {
  provider: anchor.AnchorProvider;
  payer: anchor.web3.Keypair;
  program: any;
  market: PublicKey;
  futarchyAuthority: PublicKey;
  eventAuthority: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
  baseYlpMint: PublicKey;
  quoteYlpMint: PublicKey;
  baseReserveVault: PublicKey;
  quoteReserveVault: PublicKey;
  baseAmount: bigint;
  quoteAmount: bigint;
}) {
  const baseTokenProgram = await tokenProgramForMint(params.provider.connection, params.baseMint);
  const quoteTokenProgram = await tokenProgramForMint(params.provider.connection, params.quoteMint);
  const ownerBaseAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.baseMint,
    owner: params.payer.publicKey,
    tokenProgram: baseTokenProgram,
  });
  const ownerQuoteAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.quoteMint,
    owner: params.payer.publicKey,
    tokenProgram: quoteTokenProgram,
  });
  const ownerBaseYlpAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.baseYlpMint,
    owner: params.payer.publicKey,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
  });
  const ownerQuoteYlpAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.quoteYlpMint,
    owner: params.payer.publicKey,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
  });

  await mintMockTokens({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.baseMint,
    recipient: params.payer.publicKey,
    amount: params.baseAmount,
    tokenProgram: baseTokenProgram,
  });
  await mintMockTokens({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.quoteMint,
    recipient: params.payer.publicKey,
    amount: params.quoteAmount,
    tokenProgram: quoteTokenProgram,
  });

  const signature = await params.program.methods
    .addLiquidity({
      baseDepositAmount: bnFromUnits(params.baseAmount),
      quoteDepositAmount: bnFromUnits(params.quoteAmount),
      minBaseYlpAmount: new anchor.BN(0),
      minQuoteYlpAmount: new anchor.BN(0),
    })
    .accounts({
      market: params.market,
      futarchyAuthority: params.futarchyAuthority,
      owner: params.payer.publicKey,
      baseMint: params.baseMint,
      quoteMint: params.quoteMint,
      baseYlpMint: params.baseYlpMint,
      quoteYlpMint: params.quoteYlpMint,
      baseReserveVault: params.baseReserveVault,
      quoteReserveVault: params.quoteReserveVault,
      ownerBaseAccount: ownerBaseAccount.address,
      ownerQuoteAccount: ownerQuoteAccount.address,
      ownerBaseYlpAccount: ownerBaseYlpAccount.address,
      ownerQuoteYlpAccount: ownerQuoteYlpAccount.address,
      baseYieldAccount: deriveYieldAccountAddress(
        params.program.programId,
        params.market,
        params.payer.publicKey,
        params.baseMint,
        "ylp"
      ),
      quoteYieldAccount: deriveYieldAccountAddress(
        params.program.programId,
        params.market,
        params.payer.publicKey,
        params.quoteMint,
        "ylp"
      ),
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      eventAuthority: params.eventAuthority,
      program: params.program.programId,
    })
    .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
    .rpc();
  console.log(
    `Seeded ${params.baseAmount.toString()} base units and ${params.quoteAmount.toString()} quote units`
  );
  console.log(explorerTx(signature));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
