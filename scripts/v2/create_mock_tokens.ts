import {
  TOKEN_PROGRAMS,
  createMintIfMissing,
  providerFromEnv,
  payerFromProvider,
  readState,
  writeState,
} from "./common.ts";

async function main() {
  const provider = providerFromEnv();
  const payer = payerFromProvider(provider);
  const state = readState();
  const tokenProgram =
    process.env.OMNIPAIR_V2_TOKEN_PROGRAM === "token2022"
      ? TOKEN_PROGRAMS.token2022
      : TOKEN_PROGRAMS.token;
  const decimals = Number(process.env.OMNIPAIR_V2_MOCK_DECIMALS ?? "6");
  const baseLabel = process.env.OMNIPAIR_V2_MOCK_BASE_LABEL ?? "base";
  const quoteLabel = process.env.OMNIPAIR_V2_MOCK_QUOTE_LABEL ?? "quote";

  const baseMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: baseLabel,
    decimals,
    mintAuthority: payer.publicKey,
    tokenProgram,
  });
  const quoteMint = await createMintIfMissing({
    connection: provider.connection,
    payer,
    label: quoteLabel,
    decimals,
    mintAuthority: payer.publicKey,
    tokenProgram,
  });

  state.mockMints[baseLabel] = baseMint;
  state.mockMints[quoteLabel] = quoteMint;
  writeState(state);

  console.log("V2 mock mints ready");
  console.log(`State: ${process.env.OMNIPAIR_V2_DEVNET_STATE ?? "default"}`);
  console.log(`${baseLabel}: ${baseMint.mint}`);
  console.log(`${quoteLabel}: ${quoteMint.mint}`);
  console.log(`Token program: ${tokenProgram.toBase58()}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
