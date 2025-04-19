import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    createMint, 
    getAssociatedTokenAddress, 
    createAssociatedTokenAccount,
    mintTo,
    getAccount
} from "@solana/spl-token";
import { assert } from "chai";
import { BN } from "bn.js";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

describe("Liquidity Operations", () => {
    // Configure the client to use the local cluster
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.Omnipair;
    
    // Load deployer keypair
    const rawKey = JSON.parse(fs.readFileSync('deployer_keypair', 'utf-8'));
    const deployer = Keypair.fromSecretKey(new Uint8Array(rawKey));
    
    // Test tokens and accounts
    let token0Mint: PublicKey;
    let token1Mint: PublicKey;
    let deployerToken0Account: PublicKey;
    let deployerToken1Account: PublicKey;
    let deployerLpTokenAccount: PublicKey;
    let rateModel: PublicKey;
    let pair: PublicKey;
    let lpMint: PublicKey;
    let reserve0VaultAta: PublicKey;
    let reserve1VaultAta: PublicKey;
    let collateral0VaultAta: PublicKey;
    let collateral1VaultAta: PublicKey;

    const DECIMALS = 9;
    const INITIAL_AMOUNT_0 = new BN(20 * Math.pow(10, DECIMALS)); // 20 tokens
    const INITIAL_AMOUNT_1 = new BN(10 * Math.pow(10, DECIMALS)); // 10 tokens
    const ADD_AMOUNT = new BN(5 * Math.pow(10, DECIMALS)); // 5 tokens

    before(async () => {
        // Airdrop SOL to deployer
        const signature = await provider.connection.requestAirdrop(
            deployer.publicKey,
            2 * anchor.web3.LAMPORTS_PER_SOL
        );
        await provider.connection.confirmTransaction(signature);

        // Create test token mints
        token0Mint = await createMint(
            provider.connection,
            deployer,
            deployer.publicKey,
            null,
            DECIMALS
        );
        token1Mint = await createMint(
            provider.connection,
            deployer,
            deployer.publicKey,
            null,
            DECIMALS
        );
        
        // Ensure token0 address is less than token1
        if (token0Mint.toBase58() > token1Mint.toBase58()) {
            [token0Mint, token1Mint] = [token1Mint, token0Mint];
        }

        // Create token accounts for deployer
        deployerToken0Account = await createAssociatedTokenAccount(
            provider.connection,
            deployer,
            token0Mint,
            deployer.publicKey
        );
        deployerToken1Account = await createAssociatedTokenAccount(
            provider.connection,
            deployer,
            token1Mint,
            deployer.publicKey
        );

        // Mint initial tokens to deployer
        await mintTo(
            provider.connection,
            deployer,
            token0Mint,
            deployerToken0Account,
            deployer.publicKey,
            INITIAL_AMOUNT_0.muln(2).toNumber()
        );
        await mintTo(
            provider.connection,
            deployer,
            token1Mint,
            deployerToken1Account,
            deployer.publicKey,
            INITIAL_AMOUNT_1.muln(2).toNumber()
        );

        // Create rate model
        const rateModelKeypair = Keypair.generate();
        rateModel = rateModelKeypair.publicKey;
        
        // Create rate model account
        await program.methods
            .createRateModel()
            .accounts({
                rateModel: rateModel,
                deployer: deployer.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .signers([deployer, rateModelKeypair])
            .rpc();

        // Derive PDAs
        [pair] = PublicKey.findProgramAddressSync(
            [
                Buffer.from("gamm_pair"),
                token0Mint.toBuffer(),
                token1Mint.toBuffer()
            ],
            program.programId
        );

        [lpMint] = PublicKey.findProgramAddressSync(
            [
                Buffer.from("gamm_lp_mint"),
                pair.toBuffer()
            ],
            program.programId
        );

        // Get ATAs
        reserve0VaultAta = await getAssociatedTokenAddress(token0Mint, pair, true);
        reserve1VaultAta = await getAssociatedTokenAddress(token1Mint, pair, true);
        collateral0VaultAta = await getAssociatedTokenAddress(token0Mint, pair, true);
        collateral1VaultAta = await getAssociatedTokenAddress(token1Mint, pair, true);
        deployerLpTokenAccount = await getAssociatedTokenAddress(lpMint, deployer.publicKey);
    });

    it("Initialize pair with initial liquidity", async () => {
        try {
            await program.methods
                .initializePair({
                    amount0In: INITIAL_AMOUNT_0,
                    amount1In: INITIAL_AMOUNT_1,
                    minLiquidityOut: new BN(0)
                })
                .accounts({
                    token0Mint,
                    token1Mint,
                    rateModel,
                    pair,
                    lpMint,
                    deployerToken0Account,
                    deployerToken1Account,
                    deployerLpTokenAccount,
                    reserve0VaultAta,
                    reserve1VaultAta,
                    collateral0VaultAta,
                    collateral1VaultAta,
                    deployer: deployer.publicKey,
                    systemProgram: SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    token2022Program: TOKEN_2022_PROGRAM_ID,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                })
                .signers([deployer])
                .rpc();

            // Verify initialization
            const pairAccount = await program.account.pair.fetch(pair);
            assert.ok(pairAccount.token0.equals(token0Mint));
            assert.ok(pairAccount.token1.equals(token1Mint));
            assert.ok(pairAccount.reserve0.eq(INITIAL_AMOUNT_0));
            assert.ok(pairAccount.reserve1.eq(INITIAL_AMOUNT_1));
        } catch (err) {
            console.error("Error initializing pair:", err);
            throw err;
        }
    });

    it("Add more liquidity", async () => {
        try {
            await program.methods
                .addLiquidity({
                    amount0In: ADD_AMOUNT,
                    amount1In: ADD_AMOUNT,
                    minLiquidityOut: new BN(0)
                })
                .accounts({
                    pair,
                    userToken0Account: deployerToken0Account,
                    userToken1Account: deployerToken1Account,
                    reserve0VaultAta,
                    reserve1VaultAta,
                    lpMint,
                    userLpTokenAccount: deployerLpTokenAccount,
                    user: deployer.publicKey,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    token2022Program: TOKEN_2022_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .signers([deployer])
                .rpc();

            // Verify liquidity addition
            const pairAccount = await program.account.pair.fetch(pair);
            assert.ok(pairAccount.reserve0.eq(INITIAL_AMOUNT_0.add(ADD_AMOUNT)));
            assert.ok(pairAccount.reserve1.eq(INITIAL_AMOUNT_1.add(ADD_AMOUNT)));
        } catch (err) {
            console.error("Error adding liquidity:", err);
            throw err;
        }
    });

    it("Remove liquidity", async () => {
        try {
            // Get current LP token balance
            const lpBalance = (await getAccount(provider.connection, deployerLpTokenAccount)).amount;
            const removeAmount = new BN(lpBalance.toString()).divn(2); // Remove half of LP tokens

            await program.methods
                .removeLiquidity({
                    lpAmountIn: removeAmount,
                    minAmount0Out: new BN(0),
                    minAmount1Out: new BN(0)
                })
                .accounts({
                    pair,
                    userToken0Account: deployerToken0Account,
                    userToken1Account: deployerToken1Account,
                    reserve0VaultAta,
                    reserve1VaultAta,
                    lpMint,
                    userLpTokenAccount: deployerLpTokenAccount,
                    user: deployer.publicKey,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    token2022Program: TOKEN_2022_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .signers([deployer])
                .rpc();

            // Verify liquidity removal
            const pairAccount = await program.account.pair.fetch(pair);
            const newLpBalance = (await getAccount(provider.connection, deployerLpTokenAccount)).amount;
            assert.ok(new BN(newLpBalance.toString()).eq(new BN(lpBalance.toString()).sub(removeAmount)));
        } catch (err) {
            console.error("Error removing liquidity:", err);
            throw err;
        }
    });
}); 