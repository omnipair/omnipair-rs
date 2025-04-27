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
    console.log('Starting liquidity removal...');
    
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

    // Get LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey,
        false,
        lpTokenProgram
    );

    // Get current LP token balance
    const lpBalance = (await getAccount(connection, deployerLpTokenAccount)).amount;
    const removeAmount = new BN(lpBalance.toString()).divn(2); // Remove half of LP tokens
    const minAmount0 = new BN(0); // Minimum amount of token0 to receive
    const minAmount1 = new BN(0); // Minimum amount of token1 to receive

    console.log('Removing liquidity:');
    console.log('LP Amount:', removeAmount.toString());
    console.log('Min Token0:', minAmount0.toString());
    console.log('Min Token1:', minAmount1.toString());

    // Create transaction
    const tx = await program.methods
        .removeLiquidity({
            liquidityIn: removeAmount,
            minAmount0Out: minAmount0,
            minAmount1Out: minAmount1
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
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 