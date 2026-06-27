import anchor from "@coral-xyz/anchor";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  PublicKey,
  SystemProgram,
  bnFromUnits,
  deriveFutarchyAuthorityAddress,
  deriveHlpYlpVaultAddress,
  deriveYieldAccountAddress,
  explorerTx,
  getOrCreateAta,
  mintDecimals,
  mintMockTokens,
  parseUnits,
  payerFromProvider,
  providerFromEnv,
  readState,
  tokenBalance,
  tokenProgramForMint,
  v2Program,
} from "./common.ts";
import idl from "../../target/idl/omnipair_v2.json" with { type: "json" };

async function main() {
  const provider = providerFromEnv();
  const payer = payerFromProvider(provider);
  const program = v2Program(idl, provider);
  const state = readState();
  const marketLabel = process.env.OMNIPAIR_V2_MARKET_LABEL ?? Object.keys(state.markets)[0];
  if (!marketLabel || !state.markets[marketLabel]) {
    throw new Error("No V2 market in state. Run yarn v2:bootstrap-market first.");
  }
  const market = state.markets[marketLabel];
  const marketAddress = new PublicKey(market.market);
  const marketAccount = await program.account.market.fetchNullable(marketAddress);
  if (!marketAccount) throw new Error(`Market account not found: ${market.market}`);

  console.log(`Fetched V2 yLP/hLP market ${market.market}`);
  console.log(`Base mint: ${market.baseMint}`);
  console.log(`Quote mint: ${market.quoteMint}`);
  console.log(`Base yLP mint: ${market.baseYlpMint}`);
  console.log(`Quote yLP mint: ${market.quoteYlpMint}`);
  console.log(`Base hLP mint: ${market.baseHlpMint}`);
  console.log(`Quote hLP mint: ${market.quoteHlpMint}`);
  console.log(
    `Base reserve balance: ${await tokenBalance(provider.connection, new PublicKey(market.baseReserveVault))}`
  );
  console.log(
    `Quote reserve balance: ${await tokenBalance(provider.connection, new PublicKey(market.quoteReserveVault))}`
  );

  const baseMint = new PublicKey(market.baseMint);
  const quoteMint = new PublicKey(market.quoteMint);
  const baseYlpMint = new PublicKey(market.baseYlpMint);
  const quoteYlpMint = new PublicKey(market.quoteYlpMint);
  const baseHlpMint = new PublicKey(market.baseHlpMint);
  const baseProgram = await tokenProgramForMint(provider.connection, baseMint);
  const quoteProgram = await tokenProgramForMint(provider.connection, quoteMint);
  const baseDecimals = await mintDecimals(provider.connection, baseMint);
  const futarchyAuthority = deriveFutarchyAuthorityAddress(program.programId);

  const traderBaseAccount = await getOrCreateAta({
    connection: provider.connection,
    payer,
    mint: baseMint,
    owner: payer.publicKey,
    tokenProgram: baseProgram,
  });
  const traderQuoteAccount = await getOrCreateAta({
    connection: provider.connection,
    payer,
    mint: quoteMint,
    owner: payer.publicKey,
    tokenProgram: quoteProgram,
  });

  if (process.env.OMNIPAIR_V2_SMOKE_OPEN_HEDGE !== "0") {
    const hedgeAmount = parseUnits(
      process.env.OMNIPAIR_V2_SMOKE_HEDGE_AMOUNT ?? "10",
      baseDecimals
    );
    await mintMockTokens({
      connection: provider.connection,
      payer,
      mint: baseMint,
      recipient: payer.publicKey,
      amount: hedgeAmount,
      tokenProgram: baseProgram,
    });

    const ownerBaseHlpAccount = await getOrCreateAta({
      connection: provider.connection,
      payer,
      mint: baseHlpMint,
      owner: payer.publicKey,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });
    const signature = await program.methods
      .openHedge({
        depositAmount: bnFromUnits(hedgeAmount),
        minHlpAmount: new anchor.BN(1),
      })
      .accounts({
        market: marketAddress,
        futarchyAuthority,
        owner: payer.publicKey,
        baseMint,
        quoteMint,
        baseYlpMint,
        quoteYlpMint,
        targetHlpMint: baseHlpMint,
        baseReserveVault: new PublicKey(market.baseReserveVault),
        quoteReserveVault: new PublicKey(market.quoteReserveVault),
        ownerTargetAccount: traderBaseAccount.address,
        ownerHlpAccount: ownerBaseHlpAccount.address,
        hlpBaseYlpAccount: new PublicKey(market.baseHlpBaseYlpVault),
        hlpQuoteYlpAccount: new PublicKey(market.baseHlpQuoteYlpVault),
        targetYieldAccount: deriveYieldAccountAddress(
          program.programId,
          marketAddress,
          payer.publicKey,
          baseMint,
          "hlp"
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        eventAuthority: new PublicKey(market.eventAuthority),
        program: program.programId,
      })
      .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
      .rpc();
    console.log(`Smoke hLP open sent: ${explorerTx(signature)}`);
  }

  if (process.env.OMNIPAIR_V2_SMOKE_SWAP === "0") return;

  const swapAmount = parseUnits(process.env.OMNIPAIR_V2_SMOKE_SWAP_AMOUNT ?? "1", baseDecimals);
  await mintMockTokens({
    connection: provider.connection,
    payer,
    mint: baseMint,
    recipient: payer.publicKey,
    amount: swapAmount,
    tokenProgram: baseProgram,
  });

  let builder = program.methods
    .swap({
      exactAssetIn: bnFromUnits(swapAmount),
      minAssetOut: new anchor.BN(0),
    })
    .accounts({
      market: marketAddress,
      futarchyAuthority,
      trader: payer.publicKey,
      assetInMint: baseMint,
      assetOutMint: quoteMint,
      reserveInVault: new PublicKey(market.baseReserveVault),
      reserveOutVault: new PublicKey(market.quoteReserveVault),
      feeInVault: new PublicKey(market.baseFeeVault),
      traderAssetInAccount: traderBaseAccount.address,
      traderAssetOutAccount: traderQuoteAccount.address,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      eventAuthority: new PublicKey(market.eventAuthority),
      program: program.programId,
    });

  const refreshedMarket = await program.account.market.fetch(marketAddress);
  if (refreshedMarket.baseHlpVault.hlpSupply.gtn(0)) {
    builder = builder.remainingAccounts([
      { pubkey: baseYlpMint, isWritable: true, isSigner: false },
      { pubkey: quoteYlpMint, isWritable: true, isSigner: false },
      {
        pubkey:
          refreshedMarket.baseHlpVault.baseYlpVault ??
          deriveHlpYlpVaultAddress(program.programId, marketAddress, baseHlpMint, baseYlpMint),
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey:
          refreshedMarket.baseHlpVault.quoteYlpVault ??
          deriveHlpYlpVaultAddress(program.programId, marketAddress, baseHlpMint, quoteYlpMint),
        isWritable: true,
        isSigner: false,
      },
    ]);
  }

  const signature = await builder
    .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })])
    .rpc();

  console.log(`Smoke swap sent: ${explorerTx(signature)}`);
  console.log(`Trader base balance: ${await tokenBalance(provider.connection, traderBaseAccount.address)}`);
  console.log(`Trader quote balance: ${await tokenBalance(provider.connection, traderQuoteAccount.address)}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
