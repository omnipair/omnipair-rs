import { 
    Connection, 
    PublicKey, 
    sendAndConfirmTransaction,
    Keypair,
    SystemProgram,
    SYSVAR_RENT_PUBKEY
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddress,
    createAssociatedTokenAccount
} from '@solana/spl-token';
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import { IDL } from '../target/types/omnipair.ts';
import BN from 'bn.js';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const PROGRAM_ID = new PublicKey('2BpW6PG5VjRHab2gFdWE19wKBPVUGnSpNUwqfWP1NtWV');
const RPC_URL = 'http://127.0.0.1:8899';
const TOKEN0_MINT = new PublicKey('4spNMSmiS7dMgyyfEWnKFkRYtML7MNyotxxvNLVAQEoQ');
const TOKEN1_MINT = new PublicKey('8deLbx5szhJfghjaX7Y8jrbj1c23AnyaM9oq8vZe9S64');
const RATE_MODEL = new PublicKey('7HgFGk2vGZcmjLHdhW1M9niuYe3eNoms6x8tehCDdWoe');

// Load deployer keypair from file
const deployerKeypairPath = path.join(__dirname, '..', 'deployer-keypair.json');
const deployerKeypairFile = fs.readFileSync(deployerKeypairPath, 'utf-8');
const DEPLOYER_KEYPAIR = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(deployerKeypairFile))
);

// Token accounts that already exist
const DEPLOYER_TOKEN0_ACCOUNT = new PublicKey('BVfHFrHMtBfWDKW1ve4q7Fm3M66dmj5Zg1sQnX6mvNKk');
const DEPLOYER_TOKEN1_ACCOUNT = new PublicKey('GenCsiGtXFdAxQsRLTNRwbCMBTCukuh3KAmNygFfv5xp');

async function main() {
    console.log('Starting liquidity addition...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program(IDL, PROGRAM_ID, provider);

    // Log all addresses
    console.log('Network:', RPC_URL);
    console.log('Program ID:', PROGRAM_ID.toBase58());
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        PROGRAM_ID
    );

    // Find PDA for the LP mint
    const [lpMintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_lp_mint'), pairPda.toBuffer()],
        PROGRAM_ID
    );

    // Get token program for each mint
    const token0Info = await connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await connection.getAccountInfo(TOKEN1_MINT);
    const lpMintInfo = await connection.getAccountInfo(lpMintPda);
    
    const token0Program = token0Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = token1Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const lpTokenProgram = lpMintInfo?.owner.equals(TOKEN_2022_PROGRAM_ID)
        ? TOKEN_2022_PROGRAM_ID
        : TOKEN_PROGRAM_ID;

    // Get associated token addresses for vaults
    const token0Vault = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        pairPda,
        true,
        token0Program,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true,
        token1Program,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // Create token vault accounts if they don't exist
    const token0VaultInfo = await connection.getAccountInfo(token0Vault);
    if (!token0VaultInfo) {
        console.log('Creating token0 vault account...');
        await createAssociatedTokenAccount(
            connection,
            DEPLOYER_KEYPAIR,
            TOKEN0_MINT,
            pairPda,
            { commitment: 'confirmed' },
            token0Program,
            ASSOCIATED_TOKEN_PROGRAM_ID
        );
    }

    const token1VaultInfo = await connection.getAccountInfo(token1Vault);
    if (!token1VaultInfo) {
        console.log('Creating token1 vault account...');
        await createAssociatedTokenAccount(
            connection,
            DEPLOYER_KEYPAIR,
            TOKEN1_MINT,
            pairPda,
            { commitment: 'confirmed' },
            token1Program,
            ASSOCIATED_TOKEN_PROGRAM_ID
        );
    }

    // Get or create LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey,
        false,
        lpTokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // Add liquidity
    const amount0 = new BN(50_000_000); // 50 tokens
    const amount1 = new BN(25_000_000); // 25 tokens
    const minLiquidity = new BN(1000); // Minimum liquidity

    console.log('Adding liquidity with amounts:');
    console.log('Token0:', amount0.toString());
    console.log('Token1:', amount1.toString());
    console.log('Min Liquidity:', minLiquidity.toString());

    // Create transaction
    const tx = await program.methods
        .addLiquidity({
            amount0In: amount0,
            amount1In: amount1,
            minLiquidityOut: minLiquidity
        })
        .accounts({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            token0Vault: token0Vault,
            token1Vault: token1Vault,
            token0VaultMint: TOKEN0_MINT,
            token1VaultMint: TOKEN1_MINT,
            userToken0Account: DEPLOYER_TOKEN0_ACCOUNT,
            userToken1Account: DEPLOYER_TOKEN1_ACCOUNT,
            lpMint: lpMintPda,
            userLpTokenAccount: deployerLpTokenAccount,
            tokenProgram: lpTokenProgram,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 