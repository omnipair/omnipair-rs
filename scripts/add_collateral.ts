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
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');

// Token accounts that already exist
const DEPLOYER_TOKEN0_ACCOUNT = new PublicKey(process.env.DEPLOYER_TOKEN0_ACCOUNT || '');
const DEPLOYER_TOKEN1_ACCOUNT = new PublicKey(process.env.DEPLOYER_TOKEN1_ACCOUNT || '');

async function main() {
    console.log('Starting add collateral operation...');
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
    const RATE_MODEL = pairAccount.rateModel;

    console.log('Rate Model address:', RATE_MODEL.toBase58());

    // Find PDA for the user position
    const [userPositionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_position'), pairPda.toBuffer(), DEPLOYER_KEYPAIR.publicKey.toBuffer()],
        program.programId
    );
    console.log('User Position PDA:', userPositionPda.toBase58());

    // Get token program for each mint
    const token0Info = await provider.connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await provider.connection.getAccountInfo(TOKEN1_MINT);
    
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

    // Add collateral parameters
    const collateralAmount = new BN(100_000_000); // 100 tokens
    const collateralToken0 = true; // Set to false to add token1 as collateral

    console.log('Adding collateral with parameters:');
    console.log('Amount:', collateralAmount.toString());
    console.log('Token:', collateralToken0 ? 'Token0' : 'Token1');

    // Create transaction
    const tx = await program.methods
        .addCollateral({
            amount: collateralAmount
        })
        .accountsStrict({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            userPosition: userPositionPda,
            collateralVault: collateralToken0 ? token0Vault : token1Vault,
            userCollateralTokenAccount: collateralToken0 ? DEPLOYER_TOKEN0_ACCOUNT : DEPLOYER_TOKEN1_ACCOUNT,
            collateralTokenMint: collateralToken0 ? TOKEN0_MINT : TOKEN1_MINT,
            tokenProgram: collateralToken0 ? token0Program : token1Program,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 