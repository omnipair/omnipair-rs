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
import { IDL } from '../target/types/omnipair.ts';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const PROGRAM_ID = new PublicKey('4DcEXKL6LxWNTxp3jZUrj1jjzU4VXPNMHsVs7Jp9NPb9');
const RPC_URL = 'http://127.0.0.1:8899'; // or your preferred network
const TOKEN0_MINT = new PublicKey('EJC5LVe13Pv4B3afsWXu3Nq9Hq5GcdBQQL4K8bByUkbp');
const TOKEN1_MINT = new PublicKey('GiZM66o3ZsBRrLkweYDbX2pqNzpdidBdUj3jn6CdV8Wh');
const RATE_MODEL = new PublicKey('GaKVkjkfDDb11cLxyDJXM78Zj8eH4bgznmF3y5212C5F');

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
    console.log('Starting pair initialization...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program(IDL, PROGRAM_ID, provider);

    console.log('Connected to network:', RPC_URL);
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Rate Model address:', RATE_MODEL.toBase58());
    console.log('Token0 Account:', DEPLOYER_TOKEN0_ACCOUNT.toBase58());
    console.log('Token1 Account:', DEPLOYER_TOKEN1_ACCOUNT.toBase58());

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
            pair: pairPda,
            lpMint: lpMintPda,
            token0Mint: TOKEN0_MINT,
            token1Mint: TOKEN1_MINT,
            rateModel: RATE_MODEL,
            deployerLpTokenAccount,
            systemProgram: SystemProgram.programId,
            // tokenProgram: TOKEN_2022_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
            // token2022Program: TOKEN_2022_PROGRAM_ID,
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