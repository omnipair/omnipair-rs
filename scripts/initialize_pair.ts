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
    getAssociatedTokenAddress
} from '@solana/spl-token';
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const RPC_URL = 'http://127.0.0.1:8899'; // or your preferred network
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
    console.log('Starting pair initialization...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program<Omnipair>(idl, provider);

    console.log('Connected to network:', RPC_URL);
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Rate Model address:', RATE_MODEL.toBase58());
    console.log('Token0 Account:', DEPLOYER_TOKEN0_ACCOUNT.toBase58());
    console.log('Token1 Account:', DEPLOYER_TOKEN1_ACCOUNT.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Find PDA for the LP mint
    const [lpMintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_lp_mint'), pairPda.toBuffer()],
        program.programId
    );

    console.log('Pair PDA:', pairPda.toBase58());
    console.log('LP Mint PDA:', lpMintPda.toBase58());

    // Get or create LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey
    );

    console.log('LP Token ATA:', deployerLpTokenAccount.toBase58());

    // Get token program for each mint
    const token0Program = (await connection.getAccountInfo(TOKEN0_MINT))?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = (await connection.getAccountInfo(TOKEN1_MINT))?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;

    console.log('Token0 Program:', token0Program.toBase58());
    console.log('Token1 Program:', token1Program.toBase58());

    // Create transaction
    const tx = await program.methods
        .initializePair()
        .accounts({
            deployer: DEPLOYER_KEYPAIR.publicKey,
            token0Mint: TOKEN0_MINT,
            token1Mint: TOKEN1_MINT,
            rateModel: RATE_MODEL,
            pair: pairPda,
            lpMint: lpMintPda,
            deployerLpTokenAccount,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: SYSVAR_RENT_PUBKEY,
        })
        .transaction();

    console.log('Transaction created. Sending...');

    // Send transaction
    const signature = await sendAndConfirmTransaction(
        connection,
        tx,
        [DEPLOYER_KEYPAIR]
    );

    console.log('Transaction successful!');
    console.log('Signature:', signature);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
}); 