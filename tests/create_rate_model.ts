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
      // 1. Get the factory address
      const [factoryAddress, factoryBump] = await PublicKey.findProgramAddress(
        [Buffer.from("factory"), owner.publicKey.toBuffer()],
        program.programId
      );
      console.log("Factory address:", factoryAddress.toBase58());

      // 2. Get the current registry address (index 0)
      const [registryAddress, registryBump] = await PublicKey.findProgramAddress(
        [
          Buffer.from("pair_registry"),
          factoryAddress.toBuffer(),
          new anchor.BN(0).toArrayLike(Buffer, "le", 4)
        ],
        program.programId
      );
      console.log("Registry address:", registryAddress.toBase58());

      // 3. Get the next registry address (index 1)
      const [nextRegistryAddress, nextRegistryBump] = await PublicKey.findProgramAddress(
        [
          Buffer.from("pair_registry"),
          factoryAddress.toBuffer(),
          new anchor.BN(1).toArrayLike(Buffer, "le", 4)
        ],
        program.programId
      );
      console.log("Next registry address:", nextRegistryAddress.toBase58());

      // 4. Create a rate model
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

      // 5. Create token mints for the pair (for testing)
      const token0Mint = Keypair.generate();
      const token1Mint = Keypair.generate();
      console.log("Token0 mint:", token0Mint.publicKey.toBase58());
      console.log("Token1 mint:", token1Mint.publicKey.toBase58());

      // 6. Get the pair address
      const [pairAddress, pairBump] = await PublicKey.findProgramAddress(
        [
          Buffer.from("pair"),
          token0Mint.publicKey.toBuffer(),
          token1Mint.publicKey.toBuffer()
        ],
        program.programId
      );
      console.log("Pair address:", pairAddress.toBase58());

      // 7. Create the pair using the rate model
      await program.methods
        .createPair(rateModel.publicKey)
        .accounts({
          factory: factoryAddress,
          currentRegistry: registryAddress,
          nextRegistry: nextRegistryAddress,
          token0: token0Mint.publicKey,
          token1: token1Mint.publicKey,
          pair: pairAddress,
          payer: owner.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Pair created successfully");

      // 8. Verify the pair was created correctly
      const pair = await program.account.pair.fetch(pairAddress);
      assert.ok(pair.token0.equals(token0Mint.publicKey));
      assert.ok(pair.token1.equals(token1Mint.publicKey));
      assert.ok(pair.rateModel.equals(rateModel.publicKey));

      console.log("Pair verification successful");
    } catch (err) {
      console.error("Error:", err);
      throw err;
    }
  });
}); 