import { 
    Connection, 
    Keypair,
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    createMint,
    createAssociatedTokenAccount,
    mintTo
} from '@solana/spl-token';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const RPC_URL = 'http://127.0.0.1:8899'; // or your preferred network

// Load deployer keypair from file
const deployerKeypairPath = path.join(__dirname, '..', 'deployer-keypair.json');
const deployerKeypairFile = fs.readFileSync(deployerKeypairPath, 'utf-8');
const DEPLOYER_KEYPAIR = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(deployerKeypairFile))
);

async function main() {
    console.log('Starting token deployment...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');

    console.log('Connected to network:', RPC_URL);
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());

    // Create Token0
    console.log('\nCreating Token0...');
    const token0Mint = await createMint(
        connection,
        DEPLOYER_KEYPAIR,
        DEPLOYER_KEYPAIR.publicKey,
        DEPLOYER_KEYPAIR.publicKey,
        6, // Decimals
        undefined,
        undefined,
        TOKEN_PROGRAM_ID
    );
    console.log('Token0 Mint:', token0Mint.toBase58());

    // Create Token1
    console.log('\nCreating Token1...');
    const token1Mint = await createMint(
        connection,
        DEPLOYER_KEYPAIR,
        DEPLOYER_KEYPAIR.publicKey,
        DEPLOYER_KEYPAIR.publicKey,
        6, // Decimals
        undefined,
        undefined,
        TOKEN_PROGRAM_ID
    );
    console.log('Token1 Mint:', token1Mint.toBase58());

    // Create associated token accounts for deployer
    console.log('\nCreating token accounts for deployer...');
    const deployerToken0Account = await createAssociatedTokenAccount(
        connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        DEPLOYER_KEYPAIR.publicKey
    );
    console.log('Deployer Token0 Account:', deployerToken0Account.toBase58());

    const deployerToken1Account = await createAssociatedTokenAccount(
        connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        DEPLOYER_KEYPAIR.publicKey
    );
    console.log('Deployer Token1 Account:', deployerToken1Account.toBase58());

    // Mint 1000 tokens to deployer for each token
    console.log('\nMinting tokens to deployer...');
    const mint0Amount = 20_000 * Math.pow(10, 6); // 1000 tokens with 6 decimals
    const mint1Amount = 100_000 * Math.pow(10, 6); // 1000 tokens with 6 decimals

    await mintTo(
        connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        deployerToken0Account,
        DEPLOYER_KEYPAIR,
        mint0Amount
    );
    console.log('Minted 1000 Token0 to deployer');

    await mintTo(
        connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        deployerToken1Account,
        DEPLOYER_KEYPAIR,
        mint1Amount
    );
    console.log('Minted 1000 Token1 to deployer');

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
