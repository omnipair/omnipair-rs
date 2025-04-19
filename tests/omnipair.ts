import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAccount, mintTo, getAssociatedTokenAddress, createAssociatedTokenAccount } from "@solana/spl-token";
import { assert } from "chai";
import { IDL } from "../target/types/omnipair";

describe("omnipair", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Initialize program with proper ID from Anchor.toml
  const PROGRAM_ID = new PublicKey("Hp5xcpcLe24PBrdGZkdQ9gz9VTT9xciPcumiLY7t89EY");
  const program = anchor.workspace.Omnipair;
  
  // Test accounts
  const owner = Keypair.generate();
  let factoryAddress: PublicKey;
  let factoryBump: number;
  
  // Test tokens
  let token0Mint: PublicKey;
  let token1Mint: PublicKey;
  let token0Account: PublicKey;
  let token1Account: PublicKey;
  let pairToken0Account: PublicKey;
  let pairToken1Account: PublicKey;
  let pairAddress: PublicKey;
  let pairBump: number;

  before(async () => {
    // Airdrop SOL to owner and payer
    const signature = await provider.connection.requestAirdrop(
      owner.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature);

    const payerSignature = await provider.connection.requestAirdrop(
      provider.wallet.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(payerSignature);

    // Create test token mints
    token0Mint = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      9
    );
    token1Mint = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      9
    );
    
    // Ensure token0 address is less than token1
    if (token0Mint.toBase58() > token1Mint.toBase58()) {
      [token0Mint, token1Mint] = [token1Mint, token0Mint];
    }

    // Create token accounts for owner
    token0Account = await createAssociatedTokenAccount(
      provider.connection,
      owner,
      token0Mint,
      owner.publicKey
    );
    token1Account = await createAssociatedTokenAccount(
      provider.connection,
      owner,
      token1Mint,
      owner.publicKey
    );

    // Mint some tokens to owner
    await mintTo(
      provider.connection,
      owner,
      token0Mint,
      token0Account,
      owner.publicKey,
      1_000_000_000 // 1000 tokens
    );
    await mintTo(
      provider.connection,
      owner,
      token1Mint,
      token1Account,
      owner.publicKey,
      1_000_000_000 // 1000 tokens
    );

    // Derive factory address
    [factoryAddress, factoryBump] = await PublicKey.findProgramAddress(
      [Buffer.from("factory"), owner.publicKey.toBuffer()],
      program.programId
    );

    // Derive pair address
    [pairAddress, pairBump] = await PublicKey.findProgramAddress(
      [Buffer.from("pair"), token0Mint.toBuffer(), token1Mint.toBuffer()],
      program.programId
    );

    // Get token accounts for pair
    pairToken0Account = await getAssociatedTokenAddress(token0Mint, pairAddress);
    pairToken1Account = await getAssociatedTokenAddress(token1Mint, pairAddress);
  });

  it("Initializes factory", async () => {
    try {
      await program.methods
        .initializeFactory()
        .accounts({
          factory: factoryAddress,
          owner: owner.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Verify factory state
      const factory = await program.account.factory.fetch(factoryAddress);
      assert.ok(factory.owner.equals(owner.publicKey));
      assert.equal(factory.pairCount.toNumber(), 0);
      assert.equal(factory.allPairs.length, 0);
    } catch (err) {
      console.error("Error:", err);
      throw err;
    }
  });

  it("Creates a pair", async () => {
    try {
      // Create a rate model (you'll need to implement this based on your rate model logic)
      const rateModel = Keypair.generate().publicKey;

      await program.methods
        .createPair(rateModel)
        .accounts({
          factory: factoryAddress,
          token0: token0Mint,
          token1: token1Mint,
          pair: pairAddress,
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      // Create token accounts for pair if they don't exist
      try {
        await createAssociatedTokenAccount(
          provider.connection,
          owner,
          token0Mint,
          pairAddress
        );
      } catch (e) {
        // Account might already exist
      }

      try {
        await createAssociatedTokenAccount(
          provider.connection,
          owner,
          token1Mint,
          pairAddress
        );
      } catch (e) {
        // Account might already exist
      }

      // Verify pair creation
      const factory = await program.account.factory.fetch(factoryAddress);
      assert.equal(factory.pairCount.toNumber(), 1);
      assert.equal(factory.allPairs.length, 1);
      assert.ok(factory.allPairs[0].equals(pairAddress));

      const pair = await program.account.pair.fetch(pairAddress);
      assert.ok(pair.token0.equals(token0Mint));
      assert.ok(pair.token1.equals(token1Mint));
      assert.ok(pair.rateModel.equals(rateModel));
    } catch (err) {
      console.error("Error:", err);
      throw err;
    }
  });

  it("Provides initial liquidity", async () => {
    try {
      const amount0 = new anchor.BN(100_000_000); // 100 tokens
      const amount1 = new anchor.BN(100_000_000); // 100 tokens
      const minLiquidity = new anchor.BN(1000); // Minimum liquidity

      await program.methods
        .adjustCollateral(amount0, amount1)
        .accounts({
          pair: pairAddress,
          token0: pairToken0Account,
          token1: pairToken1Account,
          userToken0: token0Account,
          userToken1: token1Account,
          user: owner.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Verify liquidity provision
      const pair = await program.account.pair.fetch(pairAddress);
      assert.equal(pair.totalCollateral0.toNumber(), amount0.toNumber());
      assert.equal(pair.totalCollateral1.toNumber(), amount1.toNumber());
      assert.ok(pair.totalSupply.gt(minLiquidity));
    } catch (err) {
      console.error("Error:", err);
      throw err;
    }
  });
}); 