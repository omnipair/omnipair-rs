import { 
    PublicKey, 
    SystemProgram,
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddress,
} from '@solana/spl-token';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import BN from 'bn.js';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');

async function main() {
    console.log('Starting liquidity addition...');
    
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

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Get pair account to get rate model
    const pairAccount = await program.account.pair.fetch(pairPda);
    console.log('Rate model address:', pairAccount.rateModel.toBase58());
    
    const RATE_MODEL = pairAccount.rateModel;

    // Find PDA for futarchy authority
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());

    // Get vault accounts (pair-owned)
    const token0Vault = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        pairPda,
        true // allowOwnerOffCurve for PDAs
    );
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true // allowOwnerOffCurve for PDAs
    );
    console.log('Token0 Vault:', token0Vault.toBase58());
    console.log('Token1 Vault:', token1Vault.toBase58());

    // Find PDA for the LP mint
    const [lpMintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_lp_mint'), pairPda.toBuffer()],
        program.programId
    );

    // Get or create LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey
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
        .accountsPartial({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            futarchyAuthority: futarchyAuthorityPda,
            token0Vault: token0Vault,
            token1Vault: token1Vault,
            token0VaultMint: TOKEN0_MINT,
            token1VaultMint: TOKEN1_MINT,
            userToken0Account: DEPLOYER_TOKEN0_ACCOUNT,
            userToken1Account: DEPLOYER_TOKEN1_ACCOUNT,
            lpMint: lpMintPda,
            userLpTokenAccount: deployerLpTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
}); 