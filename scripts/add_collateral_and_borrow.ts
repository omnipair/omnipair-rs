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
    console.log('Starting add collateral and borrow operation...');
    
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

    // Get pair account to get pair config and rate model
    const pairAccount = await program.account.pair.fetch(pairPda);
    console.log('Pair config address:', pairAccount.config.toBase58());
    console.log('Rate model address:', pairAccount.rateModel.toBase58());
    
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

    // Configuration: Choose which token to use as collateral and which to borrow
    // Set these variables to control the operation
    const useToken0AsCollateral = false; // Set to false to use token1 as collateral
    const useToken0AsBorrow = true;    // Set to true to borrow token0, false to borrow token1

    // Note: Collateral and borrow tokens are configured to be different

    // Set amounts
    const collateralAmount = new BN(10_000_000); // 10 tokens as collateral
    const borrowAmount = new BN(5_000_000);      // 5 tokens to borrow

    console.log('Add Collateral and Borrow parameters:');
    console.log('Collateral Token:', useToken0AsCollateral ? 'Token0' : 'Token1');
    console.log('Borrow Token:', useToken0AsBorrow ? 'Token0' : 'Token1');
    console.log('Collateral Amount:', collateralAmount.toString());
    console.log('Borrow Amount:', borrowAmount.toString());

    // Determine which accounts to use based on configuration
    const collateralVault = useToken0AsCollateral ? token0Vault : token1Vault;
    const borrowVault = useToken0AsBorrow ? token0Vault : token1Vault;
    const userCollateralTokenAccount = useToken0AsCollateral ? DEPLOYER_TOKEN0_ACCOUNT : DEPLOYER_TOKEN1_ACCOUNT;
    const userBorrowTokenAccount = useToken0AsBorrow ? DEPLOYER_TOKEN0_ACCOUNT : DEPLOYER_TOKEN1_ACCOUNT;
    const collateralTokenMint = useToken0AsCollateral ? TOKEN0_MINT : TOKEN1_MINT;
    const borrowTokenMint = useToken0AsBorrow ? TOKEN0_MINT : TOKEN1_MINT;
    const collateralTokenProgram = useToken0AsCollateral ? token0Program : token1Program;
    const borrowTokenProgram = useToken0AsBorrow ? token0Program : token1Program;

    // Create transaction
    console.log('Sending transaction...');
    try {
        const tx = await program.methods
            .addCollateralAndBorrow({
                collateralAmount: collateralAmount,
                borrowAmount: borrowAmount
            })
            .accountsStrict({
                user: DEPLOYER_KEYPAIR.publicKey,
                pair: pairPda,
                rateModel: RATE_MODEL,
                userPosition: userPositionPda,
                collateralVault: collateralVault,
                userCollateralTokenAccount: userCollateralTokenAccount,
                collateralTokenMint: collateralTokenMint,
                borrowVault: borrowVault,
                userBorrowTokenAccount: userBorrowTokenAccount,
                borrowTokenMint: borrowTokenMint,
                tokenProgram: collateralTokenProgram,
                token2022Program: TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
            })
            .signers([DEPLOYER_KEYPAIR])
            .rpc({ commitment: 'confirmed' });

        console.log('Transaction successful!');
        console.log('Signature:', tx);

        // Fetch and display transaction logs
        console.log('\n=== Transaction Logs ===');
        const txInfo = await provider.connection.getTransaction(tx, {
            commitment: 'confirmed',
            maxSupportedTransactionVersion: 0
        });
        
        if (txInfo?.meta?.logMessages) {
            txInfo.meta.logMessages.forEach((log, index) => {
                console.log(`${index}: ${log}`);
            });
        } else {
            console.log('No logs found in transaction');
        }

        // Fetch and display updated user position
        console.log('\n=== Updated User Position ===');
        try {
            const updatedUserPosition = await program.account.userPosition.fetch(userPositionPda);
            console.log('User Position Account:', {
                collateral0: updatedUserPosition.collateral0.toString(),
                collateral1: updatedUserPosition.collateral1.toString(),
                debt0Shares: updatedUserPosition.debt0Shares.toString(),
                debt1Shares: updatedUserPosition.debt1Shares.toString(),
                bump: updatedUserPosition.bump.toString(),
            });
        } catch (error) {
            console.log('Could not fetch updated user position:', error);
        }

    } catch (error: any) {
        console.log('Transaction failed, but showing logs...');
        console.log('Error:', error.message);
        
        // Display logs from the error object
        if (error.logs) {
            console.log('\n=== Program Logs ===');
            error.logs.forEach((log: string, index: number) => {
                console.log(`${index}: ${log}`);
            });
        }
        
        if (error.errorLogs) {
            console.log('\n=== Error Logs ===');
            error.errorLogs.forEach((log: string, index: number) => {
                console.log(`${index}: ${log}`);
            });
        }
        
        throw error; // Re-throw to maintain original behavior
    }
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
