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
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import BN from 'bn.js';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const RPC_URL = 'http://127.0.0.1:8899';
const TOKEN0_MINT = new PublicKey('GhUR1uKdtVkTnDEBF3rfhBcARptEcGCQnyA7TaKgYeF3');
const TOKEN1_MINT = new PublicKey('JCPvZK9gf6R8YmaDnMN5YUTwV8RyYiTFN4iFAnkvR1W3');
const RATE_MODEL = new PublicKey('6kJcX5Bx8uhjpLE6C8vBqSv6eq9Zc3VLoY9T1PBktGgZ');

// Load deployer keypair from file
const deployerKeypairPath = path.join(__dirname, '..', 'deployer-keypair.json');
const deployerKeypairFile = fs.readFileSync(deployerKeypairPath, 'utf-8');
const DEPLOYER_KEYPAIR = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(deployerKeypairFile))
);

// Token accounts that already exist
const DEPLOYER_TOKEN0_ACCOUNT = new PublicKey('6DRAu1N3ZsNxRRXtdbx5fRMFhDqdSz4n1vzLdQ28TRZ8');
const DEPLOYER_TOKEN1_ACCOUNT = new PublicKey('46XmvJ7Wt7PbfyWuPsgjQrXENRiNU9BFmzfb6aYPef85');

async function main() {
    console.log('Starting repay operation...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program<Omnipair>(idl, provider);

    // Log all addresses
    console.log('Network:', RPC_URL);
    console.log('Program ID:', program.programId.toBase58());
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Find PDA for the user position
    const [userPositionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_position'), pairPda.toBuffer(), DEPLOYER_KEYPAIR.publicKey.toBuffer()],
        program.programId
    );

    // Get token program for each mint
    const token0Info = await connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await connection.getAccountInfo(TOKEN1_MINT);
    
    const token0Program = token0Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = token1Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
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

    // Repay parameters
    const repayAmount = new BN(10_000_000); // 10 tokens
    const repayToken0 = true; // Set to false to repay token1

    console.log('Repaying with parameters:');
    console.log('Amount:', repayAmount.toString());
    console.log('Token:', repayToken0 ? 'Token0' : 'Token1');

    // Create transaction
    const tx = await program.methods
        .repay({
            amount: repayAmount
        })
        .accounts({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            userPosition: userPositionPda,
            tokenVault: repayToken0 ? token0Vault : token1Vault,
            userTokenAccount: repayToken0 ? DEPLOYER_TOKEN0_ACCOUNT : DEPLOYER_TOKEN1_ACCOUNT,
            vaultTokenMint: repayToken0 ? TOKEN0_MINT : TOKEN1_MINT,
            tokenProgram: repayToken0 ? token0Program : token1Program,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 