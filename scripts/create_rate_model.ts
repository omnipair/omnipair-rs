import { 
    Connection, 
    PublicKey, 
    Transaction, 
    sendAndConfirmTransaction,
    Keypair,
    SystemProgram,
    SYSVAR_RENT_PUBKEY
} from '@solana/web3.js';
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import { IDL } from '../target/types/omnipair.ts';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// Get the directory name in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Replace these with your actual values
const PROGRAM_ID = new PublicKey('4DcEXKL6LxWNTxp3jZUrj1jjzU4VXPNMHsVs7Jp9NPb9');
const RPC_URL = 'http://127.0.0.1:8899'; // or your preferred network

// Load deployer keypair from file
const deployerKeypairPath = path.join(__dirname, '..', 'deployer-keypair.json');
const deployerKeypairFile = fs.readFileSync(deployerKeypairPath, 'utf-8');
const DEPLOYER_KEYPAIR = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(deployerKeypairFile))
);

async function main() {
    console.log('Starting rate model creation...');
    
    // Setup connection and provider
    const connection = new Connection(RPC_URL, 'confirmed');
    const wallet = new Wallet(DEPLOYER_KEYPAIR);
    const provider = new AnchorProvider(connection, wallet, {});
    const program = new Program(IDL, PROGRAM_ID, provider);

    console.log('Connected to network:', RPC_URL);
    console.log('Deployer address:', DEPLOYER_KEYPAIR.publicKey.toBase58());

    // Generate a new keypair for the rate model
    const rateModelKeypair = Keypair.generate();
    console.log('Rate Model address:', rateModelKeypair.publicKey.toBase58());

    // Create transaction
    const tx = await program.methods
        .createRateModel()
        .accounts({
            rateModel: rateModelKeypair.publicKey,
            payer: DEPLOYER_KEYPAIR.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .signers([rateModelKeypair])
        .transaction();

    console.log('Transaction created. Sending...');

    // Send transaction
    const signature = await sendAndConfirmTransaction(
        connection,
        tx,
        [DEPLOYER_KEYPAIR, rateModelKeypair]
    );

    console.log('Transaction successful!');
    console.log('Signature:', signature);
    console.log('Rate Model address:', rateModelKeypair.publicKey.toBase58());
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
}); 