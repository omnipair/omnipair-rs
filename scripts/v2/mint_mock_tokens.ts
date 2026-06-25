import {
  PublicKey,
  explorerTx,
  mintMockTokens,
  parseUnits,
  providerFromEnv,
  payerFromProvider,
  readState,
} from "./common.ts";

async function main() {
  const provider = providerFromEnv();
  const payer = payerFromProvider(provider);
  const state = readState();
  const recipient = new PublicKey(
    process.argv[2] ?? process.env.OMNIPAIR_V2_TESTER_WALLET ?? payer.publicKey.toBase58()
  );
  const labels = (process.env.OMNIPAIR_V2_MINT_LABELS ?? Object.keys(state.mockMints).join(","))
    .split(",")
    .map((label) => label.trim())
    .filter(Boolean);

  if (labels.length === 0) {
    throw new Error("No mock mints found. Run yarn v2:create-mock-tokens first.");
  }

  console.log(`Minting V2 mock tokens to ${recipient.toBase58()}`);
  for (const label of labels) {
    const storedMint = state.mockMints[label];
    if (!storedMint) throw new Error(`Unknown mock mint label: ${label}`);
    const amount = parseUnits(process.env.OMNIPAIR_V2_MINT_AMOUNT ?? "1000000", storedMint.decimals);
    const result = await mintMockTokens({
      connection: provider.connection,
      payer,
      mint: new PublicKey(storedMint.mint),
      recipient,
      amount,
      tokenProgram: new PublicKey(storedMint.tokenProgram),
    });
    console.log(`${label}: ${amount.toString()} units -> ${result.associatedTokenAccount.toBase58()}`);
    console.log(explorerTx(result.signature));
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
