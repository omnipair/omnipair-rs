import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Omnipair } from "../target/types/omnipair";
import { PublicKey, Keypair, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";

describe("create_rate_model", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Omnipair as Program<Omnipair>;
  const owner = provider.wallet.payer;

  it("Creates a rate model and uses it to create a pair", async () => {
    try {
      // 1. Create a rate model
      const rateModel = Keypair.generate();
      console.log("Rate model address:", rateModel.publicKey.toBase58());

      await program.methods
        .createRateModel()
        .accounts({
          rateModel: rateModel.publicKey,
          payer: owner.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([rateModel])
        .rpc();

      console.log("Rate model created successfully");

      // 2. Create token mints for the pair (for testing)
      const token0Mint = new PublicKey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
      const token1Mint = new PublicKey("So11111111111111111111111111111111111111112");
      console.log("Token0 mint:", token0Mint.toBase58());
      console.log("Token1 mint:", token1Mint.toBase58());

      // 3. Get the pair address
      const [pairAddress, pairBump] = await PublicKey.findProgramAddress(
        [
          Buffer.from("pair"),
          token0Mint.toBuffer(),
          token1Mint.toBuffer()
        ],
        program.programId
      );
      console.log("Pair address:", pairAddress.toBase58());

      // 4. Create the pair using the rate model
      await program.methods
        .createPair(rateModel.publicKey)
        .accounts({
          token0: token0Mint,
          token1: token1Mint,
          pair: pairAddress,
          payer: owner.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Pair created successfully");

      // 5. Verify the pair was created correctly
      const pair = await program.account.pair.fetch(pairAddress);
      assert.ok(pair.token0.equals(token0Mint));
      assert.ok(pair.token1.equals(token1Mint));
      assert.ok(pair.rateModel.equals(rateModel.publicKey));

      console.log("Pair verification successful");
    } catch (err) {
      console.error("Error:", err);
      throw err;
    }
  });
}); 