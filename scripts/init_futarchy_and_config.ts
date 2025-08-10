import { 
    PublicKey, 
    SystemProgram,
} from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';
import BN from 'bn.js';
import { leU64 } from './utils/index.ts';

// Load environment variables
dotenv.config();

async function main() {
    console.log('Starting futarchy authority and pair config initialization...');
    
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

    // Generate nonce for pair config (you might want to make this configurable)
    const pairConfigNonce = 1;
    
    // Find PDA for pair config with nonce
    const [pairConfigPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair_config'), leU64(pairConfigNonce)],
        program.programId
    );
    console.log('Pair Config PDA:', pairConfigPda.toBase58());
    console.log('Pair Config Nonce:', pairConfigNonce);

    // Step 1: Initialize futarchy authority (if not already initialized)
    try {
        console.log('Initializing futarchy authority...');
        const futarchyTx = await program.methods
            .initFutarchyAuthority({
                authority: DEPLOYER_KEYPAIR.publicKey,
            })
            .accounts({
                authoritySigner: DEPLOYER_KEYPAIR.publicKey,
                futarchy_authority: futarchyAuthorityPda,
                systemProgram: SystemProgram.programId,
            })
            .signers([DEPLOYER_KEYPAIR])
            .rpc();
        console.log('Futarchy authority initialized:', futarchyTx);
    } catch (error) {
        console.log('Futarchy authority may already be initialized:', error);
    }

    // Step 2: Initialize pair config with futarchy parameters
    console.log('Initializing pair config...');
    const pairConfigTx = await program.methods
        .initPairConfig({
            futarchyFeeBps: 50, // 0.5% futarchy fee
            founderFeeBps: 30,   // 0.3% founder fee
            nonce: new BN(pairConfigNonce),
        })
        .accountsPartial({
            authoritySigner: DEPLOYER_KEYPAIR.publicKey,
            systemProgram: SystemProgram.programId,
            pairConfig: pairConfigPda,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();
    console.log('Pair config initialized:', pairConfigTx);

    console.log('Initialization successful!');
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());
    console.log('Pair Config PDA:', pairConfigPda.toBase58());
    console.log('Pair Config Nonce:', pairConfigNonce);
    console.log('Pair Config Signature:', pairConfigTx);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
