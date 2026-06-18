import * as anchor from "@coral-xyz/anchor";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  PublicKey,
  SystemProgram,
  bnFromUnits,
  createMintIfMissing,
  defaultMarketConfig,
  deriveMarketAddresses,
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
  stakePositionAddress,
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
  const provisionalMarket = deriveMarketAddresses({
    programId: program.programId,
    baseMint,
    quoteMint,
    paramsHash,
    baseClaimTokenMint: PublicKey.default,
    quoteClaimTokenMint: PublicKey.default,
  }).market;

  const baseClaimTokenMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-base-claim`,
    decimals: baseDecimals,
    mintAuthority: provisionalMarket,
    tokenProgram: TOKEN_PROGRAM_ID,
  });
  const quoteClaimTokenMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-quote-claim`,
    decimals: quoteDecimals,
    mintAuthority: provisionalMarket,
    tokenProgram: TOKEN_PROGRAM_ID,
  });
  const baseHedgeTokenMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-base-hedge`,
    decimals: baseDecimals,
    mintAuthority: provisionalMarket,
    tokenProgram: TOKEN_PROGRAM_ID,
  });
  const quoteHedgeTokenMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: `${marketLabel}-quote-hedge`,
    decimals: quoteDecimals,
    mintAuthority: provisionalMarket,
    tokenProgram: TOKEN_PROGRAM_ID,
  });

  const addresses = deriveMarketAddresses({
    programId: program.programId,
    baseMint,
    quoteMint,
    paramsHash,
    baseClaimTokenMint: new PublicKey(baseClaimTokenMint.mint),
    quoteClaimTokenMint: new PublicKey(quoteClaimTokenMint.mint),
  });
  if (!addresses.market.equals(provisionalMarket)) {
    throw new Error("Market PDA changed unexpectedly while deriving claim mint authorities");
  }

  const marketAccount = await provider.connection.getAccountInfo(addresses.market, "confirmed");
  if (!marketAccount) {
    console.log(`Initializing V2 market ${addresses.market.toBase58()}`);
    const signature = await program.methods
      .initialize({
        operator: payer.publicKey,
        manager: payer.publicKey,
        config: defaultMarketConfig(),
        paramsHash: [...paramsHash],
      })
      .accounts({
        payer: payer.publicKey,
        baseMint,
        quoteMint,
        market: addresses.market,
        baseClaimTokenMint: new PublicKey(baseClaimTokenMint.mint),
        quoteClaimTokenMint: new PublicKey(quoteClaimTokenMint.mint),
        baseHedgeTokenMint: new PublicKey(baseHedgeTokenMint.mint),
        quoteHedgeTokenMint: new PublicKey(quoteHedgeTokenMint.mint),
        baseHedgeVault: addresses.baseHedgeVault,
        quoteHedgeVault: addresses.quoteHedgeVault,
        baseReserveVault: addresses.baseReserveVault,
        quoteReserveVault: addresses.quoteReserveVault,
        baseCollateralVault: addresses.baseCollateralVault,
        quoteCollateralVault: addresses.quoteCollateralVault,
        baseInsuranceVault: addresses.baseInsuranceVault,
        quoteInsuranceVault: addresses.quoteInsuranceVault,
        baseFeeVault: addresses.baseFeeVault,
        quoteFeeVault: addresses.quoteFeeVault,
        baseStakeVault: addresses.baseStakeVault,
        quoteStakeVault: addresses.quoteStakeVault,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        eventAuthority: addresses.eventAuthority,
        program: program.programId,
      })
      .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 })])
      .rpc();
    console.log(`Initialize tx: ${explorerTx(signature)}`);
  } else {
    console.log(`Market already exists: ${addresses.market.toBase58()}`);
  }

  const storedMarket = {
    label: marketLabel,
    programId: program.programId.toBase58(),
    market: addresses.market.toBase58(),
    paramsHash: paramsHash.toString("hex"),
    baseMint: baseMint.toBase58(),
    quoteMint: quoteMint.toBase58(),
    baseClaimTokenMint: baseClaimTokenMint.mint,
    quoteClaimTokenMint: quoteClaimTokenMint.mint,
    baseHedgeTokenMint: baseHedgeTokenMint.mint,
    quoteHedgeTokenMint: quoteHedgeTokenMint.mint,
    baseReserveVault: addresses.baseReserveVault.toBase58(),
    quoteReserveVault: addresses.quoteReserveVault.toBase58(),
    baseCollateralVault: addresses.baseCollateralVault.toBase58(),
    quoteCollateralVault: addresses.quoteCollateralVault.toBase58(),
    baseInsuranceVault: addresses.baseInsuranceVault.toBase58(),
    quoteInsuranceVault: addresses.quoteInsuranceVault.toBase58(),
    baseFeeVault: addresses.baseFeeVault.toBase58(),
    quoteFeeVault: addresses.quoteFeeVault.toBase58(),
    baseStakeVault: addresses.baseStakeVault.toBase58(),
    quoteStakeVault: addresses.quoteStakeVault.toBase58(),
    baseHedgeVault: addresses.baseHedgeVault.toBase58(),
    quoteHedgeVault: addresses.quoteHedgeVault.toBase58(),
    eventAuthority: addresses.eventAuthority.toBase58(),
    seededLiquidity: state.markets[marketLabel]?.seededLiquidity ?? false,
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
  const quoteAmount = parseUnits(process.env.OMNIPAIR_V2_QUOTE_LIQUIDITY ?? "100000", quoteDecimals);
  await seedLiquiditySide({
    provider,
    payer,
    program,
    market: addresses.market,
    eventAuthority: addresses.eventAuthority,
    marketAsset: { base: {} },
    assetMint: baseMint,
    claimTokenMint: new PublicKey(baseClaimTokenMint.mint),
    reserveVault: addresses.baseReserveVault,
    amount: baseAmount,
  });
  await seedLiquiditySide({
    provider,
    payer,
    program,
    market: addresses.market,
    eventAuthority: addresses.eventAuthority,
    marketAsset: { quote: {} },
    assetMint: quoteMint,
    claimTokenMint: new PublicKey(quoteClaimTokenMint.mint),
    reserveVault: addresses.quoteReserveVault,
    amount: quoteAmount,
  });

  state.markets[marketLabel] = { ...storedMarket, seededLiquidity: true };
  writeState(state);
  console.log("V2 market bootstrap complete");
  console.log(JSON.stringify(state.markets[marketLabel], null, 2));
}

async function seedLiquiditySide(params: {
  provider: anchor.AnchorProvider;
  payer: anchor.web3.Keypair;
  program: any;
  market: PublicKey;
  eventAuthority: PublicKey;
  marketAsset: { base?: {}; quote?: {} };
  assetMint: PublicKey;
  claimTokenMint: PublicKey;
  reserveVault: PublicKey;
  amount: bigint;
}) {
  const tokenProgram = await tokenProgramForMint(params.provider.connection, params.assetMint);
  const ownerAssetAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.assetMint,
    owner: params.payer.publicKey,
    tokenProgram,
  });
  const ownerClaimAccount = await getOrCreateAta({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.claimTokenMint,
    owner: params.payer.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
  });

  await mintMockTokens({
    connection: params.provider.connection,
    payer: params.payer,
    mint: params.assetMint,
    recipient: params.payer.publicKey,
    amount: params.amount,
    tokenProgram,
  });

  const stakePosition = stakePositionAddress(
    params.program.programId,
    params.market,
    params.payer.publicKey,
    params.assetMint
  );
  const signature = await params.program.methods
    .addLiquidity({
      marketAsset: params.marketAsset,
      depositAmount: bnFromUnits(params.amount),
      minClaimAmount: new anchor.BN(0),
      maxBufferAmount: bnFromUnits(params.amount),
    })
    .accounts({
      market: params.market,
      owner: params.payer.publicKey,
      assetMint: params.assetMint,
      claimTokenMint: params.claimTokenMint,
      reserveVault: params.reserveVault,
      ownerAssetAccount: ownerAssetAccount.address,
      ownerClaimAccount: ownerClaimAccount.address,
      stakePosition,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      eventAuthority: params.eventAuthority,
      program: params.program.programId,
    })
    .rpc();
  console.log(`Seeded ${params.amount.toString()} units into ${params.assetMint.toBase58()}`);
  console.log(explorerTx(signature));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
