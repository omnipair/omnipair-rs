import { 
    Connection, 
    PublicKey, 
    Keypair,
    SystemProgram,
    SYSVAR_RENT_PUBKEY,
    LAMPORTS_PER_SOL,
    Transaction
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    NATIVE_MINT,
    getAssociatedTokenAddress,
    getAccount,
    createAssociatedTokenAccountInstruction,
    getAssociatedTokenAddressSync
} from '@solana/spl-token';
import BN from 'bn.js';
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
    console.log('Starting pair initialization...');
    
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

    // Generate a new keypair for the rate model
    const rateModelKeypair = Keypair.generate();
    console.log('Rate Model address:', rateModelKeypair.publicKey.toBase58());

    // Find PDA for the pair using correct seed prefix
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Find PDA for the LP mint using correct seed prefix
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
    const token0Program = (await provider.connection.getAccountInfo(TOKEN0_MINT))?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = (await provider.connection.getAccountInfo(TOKEN1_MINT))?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;

    console.log('Token0 Program:', token0Program.toBase58());
    console.log('Token1 Program:', token1Program.toBase58());

    // Find PDA for futarchy authority
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());

    // Get team treasury from futarchy authority account
    const futarchyAuthorityAccount = await program.account.futarchyAuthority.fetch(futarchyAuthorityPda);
    const teamTreasury = futarchyAuthorityAccount.recipients.teamTreasury;
    console.log('Team Treasury:', teamTreasury.toBase58());

    // Get WSOL account for team treasury
    const teamTreasuryWsolAccount = getAssociatedTokenAddressSync(
        NATIVE_MINT,
        teamTreasury,
        true // allowOwnerOffCurve in case team treasury is a PDA
    );
    console.log('Team Treasury WSOL Account:', teamTreasuryWsolAccount.toBase58());
    
    // Check if the team treasury WSOL account exists, if not create it
    const teamTreasuryWsolAccountInfo = await provider.connection.getAccountInfo(teamTreasuryWsolAccount);
    if (!teamTreasuryWsolAccountInfo) {
        console.log('Creating team treasury WSOL account...');
        const createWsolIx = createAssociatedTokenAccountInstruction(
            DEPLOYER_KEYPAIR.publicKey,
            teamTreasuryWsolAccount,
            teamTreasury,
            NATIVE_MINT,
            TOKEN_PROGRAM_ID
        );
        
        const tx = new Transaction().add(createWsolIx);
        const signature = await provider.sendAndConfirm(tx, [DEPLOYER_KEYPAIR]);
        console.log('Team treasury WSOL account created:', signature);
    } else {
        console.log('Team treasury WSOL account already exists');
    }

    // Get token accounts for deployer
    const deployerToken0Account = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );
    const deployerToken1Account = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );
    console.log('Deployer Token0 Account:', deployerToken0Account.toBase58());
    console.log('Deployer Token1 Account:', deployerToken1Account.toBase58());

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

    // Check deployer token balances
    const deployerToken0Info = await getAccount(provider.connection, deployerToken0Account);
    const deployerToken1Info = await getAccount(provider.connection, deployerToken1Account);
    console.log('Deployer Token0 balance:', deployerToken0Info.amount.toString());
    console.log('Deployer Token1 balance:', deployerToken1Info.amount.toString());

    // Bootstrap liquidity amounts (adjust as needed)
    const amount0In = new BN(1_000_000_000); // 1 token with 9 decimals
    const amount1In = new BN(1_000_000_000); // 1 token with 9 decimals
    const minLiquidityOut = new BN(1_000); // Minimum liquidity tokens to receive

    // Initialize the pair with all required accounts
    console.log('Initializing pair...');
    const pairTx = await program.methods
        .initialize({
            swapFeeBps: 50, // 0.5% swap fee
            halfLife: new BN(60 * 10),  // 10 minutes in seconds
            amount0In: amount0In,
            amount1In: amount1In,
            minLiquidityOut: minLiquidityOut,
        })
        .accountsPartial({
            deployer: DEPLOYER_KEYPAIR.publicKey,
            token0Mint: TOKEN0_MINT,
            token1Mint: TOKEN1_MINT,
            pair: pairPda,
            futarchyAuthority: futarchyAuthorityPda,
            rateModel: rateModelKeypair.publicKey,
            lpMint: lpMintPda,
            deployerLpTokenAccount: deployerLpTokenAccount,
            token0Vault: token0Vault,
            token1Vault: token1Vault,
            deployerToken0Account: deployerToken0Account,
            deployerToken1Account: deployerToken1Account,
            teamTreasury: teamTreasury,
            teamTreasuryWsolAccount: teamTreasuryWsolAccount,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([DEPLOYER_KEYPAIR, rateModelKeypair])
        .rpc();

    console.log('Pair initialization successful!');
    console.log('Pair Signature:', pairTx);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
}); 