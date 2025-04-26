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
    createAssociatedTokenAccount,
    getAccount
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
const PROGRAM_ID = new PublicKey('CBAu564qqqNCkJ7VxnahmPkVBRRrsY68jqXy61c3uTrG');
const RPC_URL = 'http://127.0.0.1:8899';
const TOKEN0_MINT = new PublicKey('EJC5LVe13Pv4B3afsWXu3Nq9Hq5GcdBQQL4K8bByUkbp');
const TOKEN1_MINT = new PublicKey('GiZM66o3ZsBRrLkweYDbX2pqNzpdidBdUj3jn6CdV8Wh');
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
    console.log('Starting token swap...');
    
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
        token0Program
    );
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true,
        token1Program
    );

    // Swap parameters
    const amountIn = new BN(1000000); // Amount of token0 to swap
    const minAmountOut = new BN(0); // Minimum amount of token1 to receive

    console.log('Swap parameters:');
    console.log('Amount In:', amountIn.toString());
    console.log('Min Amount Out:', minAmountOut.toString());

    // Create transaction
    const tx = await program.methods
        .swap(amountIn, minAmountOut)
        .accounts({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            tokenInVault: token0Vault,
            tokenOutVault: token1Vault,
            userTokenInAccount: DEPLOYER_TOKEN0_ACCOUNT,
            userTokenOutAccount: DEPLOYER_TOKEN1_ACCOUNT,
            tokenInMint: TOKEN0_MINT,
            tokenOutMint: TOKEN1_MINT,
            tokenProgram: token0Program,
            token2022Program: TOKEN_2022_PROGRAM_ID,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 