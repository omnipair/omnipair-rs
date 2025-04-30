import { 
    Connection, 
    Keypair,
    sendAndConfirmTransaction,
    Transaction,
    TransactionInstruction,
    PublicKey
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    createMint,
    createAssociatedTokenAccount,
    mintTo,
    getMinimumBalanceForRentExemptMint
} from '@solana/spl-token';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

async function createMintWithRetry(
    connection: Connection,
    payer: Keypair,
    mintAuthority: PublicKey,
    freezeAuthority: PublicKey | null,
    decimals: number,
    programId = TOKEN_PROGRAM_ID,
    maxRetries = 3
) {
    let lastError;
    for (let i = 0; i < maxRetries; i++) {
        try {
            const mint = await createMint(
                connection,
                payer,
                mintAuthority,
                freezeAuthority,
                decimals,
                undefined,
                undefined,
                programId
            );
            return mint;
        } catch (error) {
            lastError = error;
            console.log(`Attempt ${i + 1} failed, retrying...`);
            await new Promise(resolve => setTimeout(resolve, 2000)); // Wait 2 seconds before retry
        }
    }
    throw lastError;
}

async function main() {
    console.log('Starting token deployment...');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    const DEPLOYER_KEYPAIR = provider.wallet.payer;
    
    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }

    // Set longer confirmation timeout
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;

    console.log('Connected to network:', provider.connection.rpcEndpoint);
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());

    // Get program ID from environment
    const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!);
    console.log('Program ID:', PROGRAM_ID.toBase58());

    // Create Token0
    console.log('\nCreating Token0...');
    const token0Mint = await createMintWithRetry(
        provider.connection,
        DEPLOYER_KEYPAIR,
        PROGRAM_ID, // Use program as mint authority
        null, // No freeze authority
        6
    );
    console.log('Token0 Mint:', token0Mint.toBase58());

    // Create Token1
    console.log('\nCreating Token1...');
    const token1Mint = await createMintWithRetry(
        provider.connection,
        DEPLOYER_KEYPAIR,
        PROGRAM_ID, // Use program as mint authority
        null, // No freeze authority
        6
    );
    console.log('Token1 Mint:', token1Mint.toBase58());

    // Create associated token accounts for deployer
    console.log('\nCreating token accounts for deployer...');
    const deployerToken0Account = await createAssociatedTokenAccount(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        DEPLOYER_KEYPAIR.publicKey
    );
    console.log('Deployer Token0 Account:', deployerToken0Account.toBase58());

    const deployerToken1Account = await createAssociatedTokenAccount(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        DEPLOYER_KEYPAIR.publicKey
    );
    console.log('Deployer Token1 Account:', deployerToken1Account.toBase58());

    // Mint initial tokens to deployer for each token
    console.log('\nMinting initial tokens to deployer...');
    const mint0Amount = 20_000 * Math.pow(10, 6); // 20,000 tokens with 6 decimals
    const mint1Amount = 100_000 * Math.pow(10, 6); // 100,000 tokens with 6 decimals

    await mintTo(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        deployerToken0Account,
        DEPLOYER_KEYPAIR,
        mint0Amount
    );
    console.log('Minted initial Token0 to deployer');

    await mintTo(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        deployerToken1Account,
        DEPLOYER_KEYPAIR,
        mint1Amount
    );
    console.log('Minted initial Token1 to deployer');

    console.log('\nToken deployment completed successfully!');
    console.log('Token0 Mint:', token0Mint.toBase58());
    console.log('Token1 Mint:', token1Mint.toBase58());
    console.log('Deployer Token0 Account:', deployerToken0Account.toBase58());
    console.log('Deployer Token1 Account:', deployerToken1Account.toBase58());
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
