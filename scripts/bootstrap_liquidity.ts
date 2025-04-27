import { 
    Connection, 
    PublicKey, 
    Keypair,
    SystemProgram,
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
import { Omnipair } from '../target/types/omnipair';
import BN from 'bn.js';
import path from 'path';
import fs from 'fs';
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
    console.log('Starting liquidity bootstrapping...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program<Omnipair>(idl, provider);

    // Log all addresses
    console.log('Network:', RPC_URL);
    console.log('Program ID:', program.programId.toBase58());
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Rate Model address:', RATE_MODEL.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());
    console.log('Deployer Token0 Account:', DEPLOYER_TOKEN0_ACCOUNT.toBase58());
    console.log('Deployer Token1 Account:', DEPLOYER_TOKEN1_ACCOUNT.toBase58());

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

    console.log('Token0 Program:', token0Program.toBase58());
    console.log('Token1 Program:', token1Program.toBase58());
    console.log('LP Token Program:', lpTokenProgram.toBase58());

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

    console.log('Pair PDA:', pairPda.toBase58());
    console.log('LP Mint PDA:', lpMintPda.toBase58());
    console.log('Token0 Vault:', token0Vault.toBase58());
    console.log('Token1 Vault:', token1Vault.toBase58());

    // Create token vault accounts if they don't exist
    try {
        await createAssociatedTokenAccount(
            connection,
            DEPLOYER_KEYPAIR,
            TOKEN0_MINT,
            pairPda,
            { commitment: 'confirmed' },
            token0Program
        );
        console.log('Created Token0 Vault account');
    } catch (e) {
        console.log('Token0 Vault account already exists');
    }

    try {
        await createAssociatedTokenAccount(
            connection,
            DEPLOYER_KEYPAIR,
            TOKEN1_MINT,
            pairPda,
            { commitment: 'confirmed' },
            token1Program
        );
        console.log('Created Token1 Vault account');
    } catch (e) {
        console.log('Token1 Vault account already exists');
    }

    // Get or create LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey,
        false,
        lpTokenProgram
    );

    console.log('LP Token ATA:', deployerLpTokenAccount.toBase58());

    // Create LP token account if it doesn't exist
    try {
        await createAssociatedTokenAccount(
            connection,
            DEPLOYER_KEYPAIR,
            lpMintPda,
            DEPLOYER_KEYPAIR.publicKey,
            { commitment: 'confirmed' },
            lpTokenProgram
        );
        console.log('Created LP Token account');
    } catch (e) {
        console.log('LP Token account already exists');
    }

    // Bootstrap liquidity
    const amount0 = new BN(200_000_000); // 200
    const amount1 = new BN(100_000_000); // 100 tokens
    const minLiquidity = new BN(1000); // Minimum liquidity

    console.log('Bootstrapping with amounts:');
    console.log('Token0:', amount0.toString());
    console.log('Token1:', amount1.toString());
    console.log('Min Liquidity:', minLiquidity.toString());

    // Create transaction
    const tx = await program.methods
        .bootstrapPair({
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
            userToken0Account: DEPLOYER_TOKEN0_ACCOUNT,
            userToken1Account: DEPLOYER_TOKEN1_ACCOUNT,
            token0VaultMint: TOKEN0_MINT,
            token1VaultMint: TOKEN1_MINT,
            lpMint: lpMintPda,
            userLpTokenAccount: deployerLpTokenAccount,
            tokenProgram: lpTokenProgram,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 