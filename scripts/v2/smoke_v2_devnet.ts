import * as anchor from "@coral-xyz/anchor";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  PublicKey,
  bnFromUnits,
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

  console.log(`Fetched V2 market ${market.market}`);
  console.log(`Base mint: ${market.baseMint}`);
  console.log(`Quote mint: ${market.quoteMint}`);
  console.log(`Base reserve balance: ${await tokenBalance(provider.connection, new PublicKey(market.baseReserveVault))}`);
  console.log(`Quote reserve balance: ${await tokenBalance(provider.connection, new PublicKey(market.quoteReserveVault))}`);

  if (process.env.OMNIPAIR_V2_SMOKE_SWAP === "0") return;

  const baseMint = new PublicKey(market.baseMint);
  const quoteMint = new PublicKey(market.quoteMint);
  const baseProgram = await tokenProgramForMint(provider.connection, baseMint);
  const quoteProgram = await tokenProgramForMint(provider.connection, quoteMint);
  const baseDecimals = await mintDecimals(provider.connection, baseMint);
  const swapAmount = parseUnits(process.env.OMNIPAIR_V2_SMOKE_SWAP_AMOUNT ?? "1", baseDecimals);

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
  await mintMockTokens({
    connection: provider.connection,
    payer,
    mint: baseMint,
    recipient: payer.publicKey,
    amount: swapAmount,
    tokenProgram: baseProgram,
  });

  const signature = await program.methods
    .swap({
      assetIn: { base: {} },
      exactAssetIn: bnFromUnits(swapAmount),
      minAssetOut: new anchor.BN(0),
    })
    .accounts({
      market: marketAddress,
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
    })
    .rpc();

  console.log(`Smoke swap sent: ${explorerTx(signature)}`);
  console.log(`Trader base balance: ${await tokenBalance(provider.connection, traderBaseAccount.address)}`);
  console.log(`Trader quote balance: ${await tokenBalance(provider.connection, traderQuoteAccount.address)}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
