import { 
    PublicKey, 
    SystemProgram,
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddress,
    createAssociatedTokenAccount
} from '@solana/spl-token';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import BN from 'bn.js';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';
import { leU64 } from './utils/index.ts';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');

async function main() {
    console.log('Starting pair initialization and bootstrap...');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    const DEPLOYER_KEYPAIR = provider.wallet.payer;
    
    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }

    // Set proper commitment levels
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;

    const DEPLOYER_TOKEN0_ACCOUNT = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );
    
    const DEPLOYER_TOKEN1_ACCOUNT = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );

    // Log all addresses
    console.log('Network:', provider.connection.rpcEndpoint);
    console.log('Program ID:', program.programId.toBase58());
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());
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

    // Generate a new keypair for the rate model
    const rateModelKeypair = anchor.web3.Keypair.generate();
    console.log('Rate Model address:', rateModelKeypair.publicKey.toBase58());

    // Get pair config PDA
    const pairConfigNonce = 1;
    const [pairConfigPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair_config'), leU64(pairConfigNonce)],
        program.programId
    );
    console.log('Pair Config PDA:', pairConfigPda.toBase58());

    // Get token program for each mint
    const token0Info = await provider.connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await provider.connection.getAccountInfo(TOKEN1_MINT);
    
    const token0Program = token0Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = token1Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;

    console.log('Token0 Program:', token0Program.toBase58());
    console.log('Token1 Program:', token1Program.toBase58());

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

    // Get or create LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey
    );

    console.log('Pair PDA:', pairPda.toBase58());
    console.log('LP Mint PDA:', lpMintPda.toBase58());
    console.log('Token0 Vault:', token0Vault.toBase58());
    console.log('Token1 Vault:', token1Vault.toBase58());
    console.log('LP Token ATA:', deployerLpTokenAccount.toBase58());

    // Bootstrap liquidity amounts
    const amount0 = new BN(9_000_000); // 90 tokens (6 decimals)
    const amount1 = new BN(20_000_000); // 200 tokens (6 decimals)
    const minLiquidity = new BN(1000); // Minimum liquidity

    console.log('Initializing and bootstrapping with amounts:');
    console.log('Token0:', amount0.toString());
    console.log('Token1:', amount1.toString());
    console.log('Min Liquidity:', minLiquidity.toString());

    // Create transaction for initialize and bootstrap
    const tx = await program.methods
        .initialize({
            swapFeeBps: 50, // 0.5% swap fee
            halfLife: new BN(60 * 10),  // 10 minutes in seconds
            poolDeployerFeeBps: 10, // 0.1% pool deployer fee
            amount0In: amount0,
            amount1In: amount1,
            minLiquidityOut: minLiquidity
        })
        .accountsPartial({
            deployer: DEPLOYER_KEYPAIR.publicKey,
            token0Mint: TOKEN0_MINT,
            token1Mint: TOKEN1_MINT,
            pair: pairPda,
            pairConfig: pairConfigPda,
            rateModel: rateModelKeypair.publicKey,
            lpMint: lpMintPda,
            deployerLpTokenAccount: deployerLpTokenAccount,
            token0Vault: token0Vault,
            token1Vault: token1Vault,
            deployerToken0Account: DEPLOYER_TOKEN0_ACCOUNT,
            deployerToken1Account: DEPLOYER_TOKEN1_ACCOUNT,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([DEPLOYER_KEYPAIR, rateModelKeypair])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
