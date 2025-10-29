import { 
    PublicKey, 
    SystemProgram,
} from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

async function main() {
    console.log('Starting futarchy authority initialization...');
    
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

    // Find PDA for futarchy authority
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());

    // Step 1: Initialize futarchy authority (if not already initialized)
    try {
        console.log('Initializing futarchy authority...');
        
        // Define treasury accounts and their percentages (must sum to 100%)
        const futarchyTreasury = new PublicKey(process.env.FUTARCHY_TREASURY_ADDRESS || DEPLOYER_KEYPAIR.publicKey.toBase58());
        const buybacksVault = new PublicKey(process.env.BUYBACKS_VAULT_ADDRESS || DEPLOYER_KEYPAIR.publicKey.toBase58());
        const teamTreasury = new PublicKey(process.env.TEAM_TREASURY_ADDRESS || DEPLOYER_KEYPAIR.publicKey.toBase58());
        
        console.log('Futarchy Treasury (10%):', futarchyTreasury.toBase58());
        console.log('Buybacks Vault (20%):', buybacksVault.toBase58());
        console.log('Team Treasury (70%):', teamTreasury.toBase58());
        
        const futarchyTx = await program.methods
            .initFutarchyAuthority({
                authority: DEPLOYER_KEYPAIR.publicKey,
                swapBps: 100, // 1% swap fee
                interestBps: 50, // 0.5% interest fee
                futarchyTreasury: futarchyTreasury,
                futarchyTreasuryBps: 1000, // 10%
                buybacksVault: buybacksVault,
                buybacksVaultBps: 2000, // 20%
                teamTreasury: teamTreasury,
                teamTreasuryBps: 7000, // 70%
            })
            .accounts({
                deployer: DEPLOYER_KEYPAIR.publicKey,
                futarchyAuthority: futarchyAuthorityPda,
                systemProgram: SystemProgram.programId,
            })
            .signers([DEPLOYER_KEYPAIR])
            .rpc();
        console.log('Futarchy authority initialized:', futarchyTx);
    } catch (error) {
        console.log('Futarchy authority may already be initialized:', error);
    }

    console.log('Initialization successful!');
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
