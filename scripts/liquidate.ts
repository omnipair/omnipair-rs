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
} from '@solana/spl-token';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');

async function main() {
    console.log('Starting liquidation operation...');
    
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

    console.log('Connected to network:', provider.connection.rpcEndpoint);
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());

    const userPublicKey = new PublicKey('C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds');
    console.log('User address:', userPublicKey.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Get pair account to get rate model
    const pairAccount = await program.account.pair.fetch(pairPda);
    console.log('Pair total debt0:', pairAccount.totalDebt0.toString());
    console.log('Pair total debt1:', pairAccount.totalDebt1.toString());
    console.log('Pair total debt0 shares:', pairAccount.totalDebt0Shares.toString());
    console.log('Pair total debt1 shares:', pairAccount.totalDebt1Shares.toString());
    const RATE_MODEL = pairAccount.rateModel;

    console.log('Rate Model address:', RATE_MODEL.toBase58());

    // Find PDA for the user position
    const [userPositionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_position'), pairPda.toBuffer(), userPublicKey.toBuffer()],
        program.programId
    );
    console.log('User position PDA:', userPositionPda.toBase58());
    const userPositionAccount = await program.account.userPosition.fetch(userPositionPda);
    console.log('User position account:', {
        collateral0: userPositionAccount.collateral0.toString(),
        collateral1: userPositionAccount.collateral1.toString(),
        debt0Shares: userPositionAccount.debt0Shares.toString(),
        debt1Shares: userPositionAccount.debt1Shares.toString(),
    });

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

    // Liquidation parameters
    const liquidateToken0 = true; // Set to false to liquidate token1 position

    console.log('Liquidating with parameters:');
    console.log('Token:', liquidateToken0 ? 'Token0' : 'Token1');

    // Create transaction
    const tx = await program.methods
        .liquidate()
        .accountsStrict({
            payer: DEPLOYER_KEYPAIR.publicKey,
            positionOwner: userPublicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            userPosition: userPositionPda,
            collateralVault: liquidateToken0 ? token0Vault : token1Vault,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 